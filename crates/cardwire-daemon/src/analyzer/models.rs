use aya::maps::{HashMap as AyaHashMap, RingBuf};
use aya_log::EbpfLogger;
use cardwire_ebpf_userspace::EbpfBlocker;
use log::{Log, debug, error, info, warn};
use std::{
    collections::{HashMap, HashSet, VecDeque}, fs, ptr, sync::{Arc, OnceLock}, time::SystemTime
};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock, Semaphore, mpsc, oneshot}, task, time::Instant
};
use zbus::object_server::SignalEmitter;

use crate::{
    analyzer::{
        dynamic_analysis::{check_env, get_app_id_wayland_with_retry, get_steam_app_id}, helpers::{
            comm_to_string, get_real_process_name, is_proc_still_alive, normalized_candidates
        }, static_analysis::{self, AppMetadata, watch_fdo_folders}
    }, file::{DbusAppMetadata, GpuPolicy}, interface::{LogEntry, LoggerInterfaceSignals, SmartPolicyInterface}
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ExecEvent {
    pub pid: u32,
    pub mode: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ReportEvent {
    pub pid: u32,
    pub gpu_id: u32,
    pub comm: [u8; 16],
}

#[derive(Debug, Copy, Clone)]
enum PidType {
    Allowed,
    Forced,
}

#[derive(Clone)]
pub struct CardwireAnalyzer {
    exec_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    report_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    pid_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
    forced_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
    ebpf_logger: Arc<Mutex<AsyncFd<EbpfLogger<&'static dyn Log>>>>,
    xdg_list: Arc<RwLock<HashMap<String, AppMetadata>>>,
    xdg_folders: Vec<std::path::PathBuf>,
    db_cache: Arc<RwLock<HashMap<String, GpuPolicy>>>,
    pending_discoveries: Arc<Mutex<HashSet<String>>>,
    db_tx: mpsc::Sender<(String, AppMetadata, oneshot::Sender<bool>)>,
    report_vec: Arc<RwLock<VecDeque<LogEntry>>>,
    reported_pids: Arc<RwLock<HashSet<u32>>>,
    report_semaphore: Arc<Semaphore>,
    signal: Arc<OnceLock<SignalEmitter<'static>>>,
    new_app_signal: Arc<OnceLock<SignalEmitter<'static>>>,
}

// Bound the number of concurrent report tasks
const REPORT_SEMAPHORE_PERMITS: usize = 32;
// Max entries kept in the report history
const MAX_REPORT_ENTRIES: usize = 4096;

impl CardwireAnalyzer {
    pub async fn build(
        blocker: Arc<RwLock<EbpfBlocker>>,
        report_vec: Arc<RwLock<VecDeque<LogEntry>>>,
        signal: Arc<OnceLock<SignalEmitter<'static>>>,
        db_cache: Arc<RwLock<HashMap<String, GpuPolicy>>>,
        db_tx: mpsc::Sender<(String, AppMetadata, oneshot::Sender<bool>)>,
        new_app_signal: Arc<OnceLock<SignalEmitter<'static>>>,
    ) -> anyhow::Result<CardwireAnalyzer> {
        let mut blocker = blocker.write().await;
        let exec_ring = blocker.get_exec_ring()?;
        let report_ring = blocker.get_report_ring()?;
        let pid_map = Arc::clone(&blocker.pid_map);
        let forced_map = Arc::clone(&blocker.forced_map);
        let ebpf_logger = blocker.get_ebpf_logger()?;

        let exec_ring = AsyncFd::new(exec_ring)?;
        let report_ring = AsyncFd::new(report_ring)?;

        // Now Rwlock -> Arc
        let exec_ring = Arc::new(Mutex::new(exec_ring));
        let report_ring = Arc::new(Mutex::new(report_ring));
        let ebpf_logger: Arc<Mutex<AsyncFd<EbpfLogger<&'static dyn Log>>>> =
            Arc::new(Mutex::new(ebpf_logger));

        let xdg_res = static_analysis::get_fdo_apps().await?;

        let xdg_list = Arc::new(RwLock::new(xdg_res.0));

        let xdg_folders: Vec<std::path::PathBuf> = xdg_res.1;

        Ok(CardwireAnalyzer {
            exec_ring,
            report_ring,
            pid_map,
            forced_map,
            ebpf_logger,
            xdg_list,
            xdg_folders,
            db_cache,
            pending_discoveries: Arc::new(Mutex::new(HashSet::new())),
            db_tx,
            report_vec,
            reported_pids: Arc::new(RwLock::new(HashSet::new())),
            report_semaphore: Arc::new(Semaphore::new(REPORT_SEMAPHORE_PERMITS)),
            signal,
            new_app_signal,
        })
    }
    pub async fn run(self) -> anyhow::Result<()> {
        // Clone the Arcs and Sender to move into the background task
        let exec_arc = self.exec_ring.clone();
        let logger_arc = self.ebpf_logger.clone();

        // Lock the buffers once
        let mut exec_ring = exec_arc.lock().await;

        let shared_self = Arc::new(self);

        // Spawn a thread that will watch the xdg folders and update the list when a new app is
        // installed
        let cloned_xdg_list = shared_self.xdg_list.clone();
        let cloned_xdg_folders = shared_self.xdg_folders.clone();

        task::spawn(async move { watch_fdo_folders(cloned_xdg_folders, cloned_xdg_list).await });

        // spawn the ebpf-logger in it's own thread
        task::spawn(async move {
            let mut ebpf_logger = logger_arc.lock().await;
            loop {
                let mut guard = match ebpf_logger.readable_mut().await {
                    Ok(guard) => guard,
                    Err(err) => {
                        error!("failed to get logger guard: {}", err);
                        return;
                    }
                };
                guard.get_inner_mut().flush();
                guard.clear_ready();
            }
        });

        // spawn the blocked event report in it's own thread
        let shared_self_report = Arc::clone(&shared_self);
        task::spawn(async move { shared_self_report.report_logger().await });

        loop {
            if let Ok(mut guard) = exec_ring.ready_mut(Interest::READABLE).await
                && guard.ready().is_readable()
            {
                while let Some(item) = guard.get_inner_mut().next() {
                    if item.len() < std::mem::size_of::<ExecEvent>() {
                        debug!("Skipping malformed exec event. Size: {}", item.len());
                        continue;
                    }
                    let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) };
                    let this = Arc::clone(&shared_self);
                    task::spawn(async move { this.spawn_exec_analyzer(event).await });
                }
                guard.clear_ready();
            }
        }
    }

    async fn spawn_exec_analyzer(&self, event: ExecEvent) -> () {
        let time = Instant::now();
        let pid_map = self.pid_map.read().await;
        if pid_map.get(&event.pid, 0).is_ok() {
            return;
        }
        drop(pid_map);
        let real_app_name = match get_real_process_name(event.pid) {
            Some(name) => name,
            None => return,
        };
        if let Some(result) = self
            .evaluate_app(event.pid, &real_app_name, event.mode)
            .await
            && result.0
        {
            match result.1 {
                PidType::Forced => {
                    info!("FORCE: pid: {} process: {} ", event.pid, real_app_name);
                    let mut forced_map = self.forced_map.write().await;
                    if let Err(e) = forced_map.insert(event.pid, result.2, 0) {
                        warn!("Failed to insert into eBPF map: {}", e);
                    }
                }
                PidType::Allowed => {
                    info!(
                        "ALLOW: pid: {} process: {} in {}us",
                        event.pid,
                        real_app_name,
                        time.elapsed().as_micros()
                    );
                    let mut pid_map = self.pid_map.write().await;
                    if let Err(e) = pid_map.insert(event.pid, result.1 as u32, 0) {
                        warn!("Failed to insert into eBPF map: {}", e);
                    }
                }
            }
        }
    }

    async fn report_logger(&self) -> () {
        let report_arc = self.report_ring.clone();
        let mut report_ring = report_arc.lock().await;
        let report_vec = self.report_vec.clone();

        // Used to prevent duplicated logs burst
        let reported_pids_arc = self.reported_pids.clone();
        let report_semaphore = self.report_semaphore.clone();
        loop {
            let mut guard = match report_ring.ready_mut(Interest::READABLE).await {
                Ok(guard) => guard,
                Err(err) => {
                    error!("failed to get report logger guard: {}", err);
                    return;
                }
            };
            while let Some(item) = guard.get_inner_mut().next() {
                if item.len() < std::mem::size_of::<ReportEvent>() {
                    warn!("Skipping malformed report event. Size: {}", item.len());
                    continue;
                }
                let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) };
                // only log if we didn't see the pid recently
                {
                    let mut reported_pids = reported_pids_arc.write().await;
                    if reported_pids.contains(&event.pid) {
                        continue;
                    } else {
                        reported_pids.insert(event.pid);
                    }
                }
                let event_comm_str = comm_to_string(event.comm);
                // Bound the number of concurrent report tasks, this prevent exausting the process
                // FD limits
                if let Ok(permit) = report_semaphore.clone().acquire_owned().await {
                    // Spawn in another task to prevent blocking the report logger while
                    // fetching informations about this process
                    let report_vec = report_vec.clone();
                    let signal = self.signal.clone();
                    let gpu_id = event.gpu_id;
                    task::spawn(async move {
                        let _permit = permit;
                        if let Some(app_id) = get_app_id_wayland_with_retry(event.pid).await {
                            report_blocked(
                                report_vec,
                                signal,
                                event.pid,
                                gpu_id,
                                event_comm_str,
                                app_id,
                            )
                            .await;
                        } else if let Some(process_name) = get_real_process_name(event.pid) {
                            report_blocked(
                                report_vec,
                                signal,
                                event.pid,
                                gpu_id,
                                process_name,
                                String::new(),
                            )
                            .await;
                        } else if is_proc_still_alive(event.pid) {
                            // we check if the proc is still here to not log noise caused by fish
                            report_blocked(
                                report_vec,
                                signal,
                                event.pid,
                                gpu_id,
                                event_comm_str,
                                String::new(),
                            )
                            .await;
                        }
                    });
                }
            }
            guard.clear_ready();
        }
    }

    /// Default app are blocked, try to find if it's a game or a gpu intensive app, the u8 is the
    /// gpu id
    async fn evaluate_app(&self, pid: u32, comm: &str, mode: u8) -> Option<(bool, PidType, u32)> {
        let path = format!("/proc/{}/environ", pid);
        let environ = match fs::read(path) {
            Ok(content) => content,
            Err(_) => return None,
        };
        // First check CARDWIRE_ALLOW, if None continue
        if let Some(allow) = check_env("CARDWIRE_ALLOW", &environ) {
            return Some((allow == 1, PidType::Allowed, 0));
        }
        if let Some(value) = check_env("CARDWIRE_FORCE_DGPU", &environ) {
            return Some((value == 1, PidType::Forced, value));
        }
        if let Some(value) = check_env("CARDWIRE_FORCE_GPU", &environ) {
            return Some((true, PidType::Forced, value));
        }

        // If manual mode, do not process app discovery or database policies
        if mode == 2 {
            return None;
        }

        // Check the database now, we can take our time since if we reached it, the app would've
        // been blocked
        let mut lookup_name = comm.to_lowercase();
        if lookup_name.contains("xdg-desktop-portal") {
            return None;
        }
        if let Some(steam_app) = get_steam_app_id(&environ) {
            lookup_name = steam_app;
        }
        {
            let db = self.db_cache.read().await;
            if let Some(policy) = db.get(&lookup_name) {
                // For now this only work for the smart mode, will need to find a way to get the GPU
                // id to make it compatible with manual mode
                match policy {
                    GpuPolicy::Blocked => return None,
                    GpuPolicy::Allowed => return Some((true, PidType::Allowed, 0)),
                }
            }
        }

        {
            let xdg_list = self.xdg_list.read().await;
            for candidate in normalized_candidates(&lookup_name) {
                if let Some(meta) = xdg_list.get(&candidate) {
                    let meta = meta.clone();
                    drop(xdg_list);
                    self.discover_app(&lookup_name, meta).await;
                    return Some((false, PidType::Allowed, 0));
                }
            }
            if let Some((_key, meta)) = xdg_list
                .iter()
                .find(|(key, _)| key.len() >= 3 && lookup_name.starts_with(key.as_str()))
            {
                let meta = meta.clone();
                drop(xdg_list);
                self.discover_app(&lookup_name, meta).await;
                return Some((false, PidType::Allowed, 0));
            }
        }
        // Fallback for steam games
        if let Some(app_id) = lookup_name.strip_prefix("steam_app_") {
            let meta = AppMetadata {
                display_name: format!("Steam Game {}", app_id),
                desktop_file_id: None,
                icon_name: Some(format!("steam_icon_{}", app_id)),
            };

            self.discover_app(&lookup_name, meta).await;
            info!("Discovered Steam Game {}, blocked by default.", app_id);
            return Some((false, PidType::Allowed, 0));
        }
        None
    }

    /// Persist a newly discovered app in the database and mirror it in the cache
    async fn discover_app(&self, lookup_name: &str, meta: AppMetadata) {
        // Allow only one persistence request per unknown app at a time, skip
        // duplicate discoveries while a request is still pending
        {
            let mut pending = self.pending_discoveries.lock().await;
            if !pending.insert(lookup_name.to_string()) {
                return;
            }
        }

        let dbus_meta = DbusAppMetadata::from_app_metadata(&meta, GpuPolicy::Blocked as u32);
        // Mirror in the cache regardless of the outcome, the app is blocked by default
        // and this prevents re-discovering it on every new process spawn
        self.db_cache
            .write()
            .await
            .insert(lookup_name.to_string(), GpuPolicy::Blocked);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let res = self
            .db_tx
            .send((lookup_name.to_string(), meta, reply_tx))
            .await;
        match res {
            Ok(_) => match reply_rx.await {
                Ok(true) => {
                    if let Some(emitter) = self.new_app_signal.get()
                        && let Err(e) = SmartPolicyInterface::new_app_added(
                            emitter,
                            (lookup_name.to_string(), dbus_meta),
                        )
                        .await
                    {
                        error!("failed to emit process_blocked_changed: {}", e);
                    }
                    info!(
                        "Discovered a new app: {}, adding to the database",
                        lookup_name
                    );
                }
                // Duplicate entry or write failure, nothing to do
                Ok(false) => {}
                Err(err) => {
                    error!("DB worker dropped the reply for {}: {}", lookup_name, err)
                }
            },
            Err(err) => {
                error!("Couldn't send new app to DB rw: {}", err)
            }
        }

        // Remove the pending entry on every exit path
        self.pending_discoveries.lock().await.remove(lookup_name);
    }
}

/// Record a blocked process in the report history and notify listeners
async fn report_blocked(
    report_vec: Arc<RwLock<VecDeque<LogEntry>>>,
    signal: Arc<OnceLock<SignalEmitter<'static>>>,
    pid: u32,
    gpu_id: u32,
    name: String,
    wayland_app_id: String,
) {
    let log_entry = LogEntry {
        timestamp: SystemTime::now(),
        pid,
        comm: name.clone(),
        gpu_id,
        wayland_app_id: wayland_app_id.clone(),
    };
    {
        let mut report_vec = report_vec.write().await;
        report_vec.push_back(log_entry.clone());
        while report_vec.len() > MAX_REPORT_ENTRIES {
            report_vec.pop_front();
        }
    }
    if wayland_app_id.is_empty() {
        info!(
            "{}[{}] tried to access GPU {} (blocked by cardwire)",
            name, pid, gpu_id
        );
    } else {
        info!(
            "{}[{}] tried to access GPU {} (blocked by cardwire)",
            wayland_app_id, pid, gpu_id
        );
    }
    if let Some(emitter) = signal.get()
        && let Err(e) = LoggerInterfaceSignals::process_blocked_changed(emitter, log_entry).await
    {
        error!("failed to emit process_blocked_changed: {}", e);
    }
}

// TESTS

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::helpers::comm_to_string;
    use std::ptr;

    #[test]
    fn test_event_deserialization_from_valid_bytes() {
        let item: Vec<u8> = vec![
            0x01, 0x00, 0x00, 0x00, // pid = 1
        ];
        assert!(item.len() >= std::mem::size_of::<ExecEvent>());
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) };
        assert_eq!(event.pid, 1);
    }

    #[test]
    fn test_event_deserialization_rejects_undersized_buffer() {
        let item: Vec<u8> = vec![0x01, 0x00, 0x00]; // 3 bytes, Event needs 4
        assert!(item.len() < std::mem::size_of::<ExecEvent>());
    }

    #[test]
    fn test_event_deserialization_with_large_pid() {
        // pid = 0xFFFFFFFF (u32::MAX)
        let item: Vec<u8> = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) };
        assert_eq!(event.pid, u32::MAX);
    }

    // ── ReportEvent ──────────────────────────────────────────────────

    #[test]
    fn test_report_event_deserialization_from_valid_bytes() {
        // ReportEvent: pid (4 bytes) + gpu_id (4 bytes) + comm (16 bytes)
        let mut item: Vec<u8> = vec![
            0x39, 0x05, 0x00, 0x00, // pid = 1337
            0x01, 0x00, 0x00, 0x00, // gpu_id = 1
        ];
        item.extend_from_slice(b"test_comm\0\0\0\0\0\0\0");
        assert!(item.len() >= std::mem::size_of::<ReportEvent>());
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) };
        assert_eq!(event.pid, 1337);
        assert_eq!(event.gpu_id, 1);
        assert_eq!(&event.comm, b"test_comm\0\0\0\0\0\0\0");
        assert_eq!(comm_to_string(event.comm), "test_comm");
    }

    #[test]
    fn test_report_event_deserialization_rejects_undersized_buffer() {
        // ReportEvent needs 24 bytes, give only 3
        let item: Vec<u8> = vec![0u8; 3];
        assert!(item.len() < std::mem::size_of::<ReportEvent>());
    }

    #[test]
    fn test_report_event_rejects_old_4_byte_layout() {
        // The old ReportEvent layout was only 4 bytes (pid), the ring reader
        // must reject it now that the event carries gpu_id and comm
        let item: Vec<u8> = vec![
            0x39, 0x05, 0x00, 0x00, // pid = 1337
        ];
        assert!(item.len() < std::mem::size_of::<ReportEvent>());
    }

    #[test]
    fn test_report_event_deserialization_pid_extraction() {
        let mut item: Vec<u8> = vec![
            0x01, 0x00, 0x00, 0x00, // pid = 1
            0x02, 0x00, 0x00, 0x00, // gpu_id = 2
        ];
        item.extend_from_slice(&[0u8; 16]);
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) };
        assert_eq!(event.pid, 1);
        assert_eq!(event.gpu_id, 2);
        assert_eq!(&event.comm, &[0u8; 16]);
    }
}
