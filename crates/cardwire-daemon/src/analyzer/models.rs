use aya::maps::{HashMap as AyaHashMap, RingBuf};
use aya_log::EbpfLogger;
use cardwire_ebpf_userspace::EbpfBlocker;
use log::{Log, debug, error, info, warn};
use std::{
    collections::{HashMap, HashSet, VecDeque}, fs, path::Path, ptr, sync::Arc, time::SystemTime
};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock, Semaphore, mpsc}, task, time::Instant
};
use zbus::object_server::SignalEmitter;

use crate::{
    analyzer::{
        dynamic_analysis::{
            check_env, check_gpu_env, check_steam_environ, get_app_id_wayland_with_retry
        }, static_analysis::{self, AppMetadata}
    }, file::GpuPolicy, interface::{LogEntry, LoggerInterfaceSignals}
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ExecEvent {
    pub pid: u32,
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
    db_cache: Arc<RwLock<HashMap<String, GpuPolicy>>>,
    db_tx: mpsc::Sender<(String, AppMetadata)>,
    report_vec: Arc<RwLock<VecDeque<LogEntry>>>,
    reported_pids: Arc<RwLock<HashSet<u32>>>,
    report_semaphore: Arc<Semaphore>,
    signal: Option<SignalEmitter<'static>>,
}

// Bound the number of concurrent report tasks
const REPORT_SEMAPHORE_PERMITS: usize = 32;
// Max entries kept in the report history
const MAX_REPORT_ENTRIES: usize = 4096;

impl CardwireAnalyzer {
    pub async fn build(
        blocker: Arc<RwLock<EbpfBlocker>>,
        report_vec: Arc<RwLock<VecDeque<LogEntry>>>,
        signal: Option<SignalEmitter<'static>>,
        db_cache: Arc<RwLock<HashMap<String, GpuPolicy>>>,
        db_tx: mpsc::Sender<(String, AppMetadata)>,
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

        let xdg_result = static_analysis::get_fdo_apps().await?;
        let xdg_list = Arc::new(RwLock::new(xdg_result.0));

        Ok(CardwireAnalyzer {
            exec_ring,
            report_ring,
            pid_map,
            forced_map,
            ebpf_logger,
            xdg_list,
            db_cache,
            db_tx,
            report_vec,
            reported_pids: Arc::new(RwLock::new(HashSet::new())),
            report_semaphore: Arc::new(Semaphore::new(REPORT_SEMAPHORE_PERMITS)),
            signal,
        })
    }
    pub async fn run(self) -> anyhow::Result<()> {
        // Clone the Arcs and Sender to move into the background task
        let exec_arc = self.exec_ring.clone();
        let logger_arc = self.ebpf_logger.clone();

        // Lock the buffers once
        let mut exec_ring = exec_arc.lock().await;

        let shared_self = Arc::new(self);

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
        if let Some(result) = self.evaluate_app(event.pid, &real_app_name).await
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
                            // we check if the proc is still here to not log noise caused by fish
                        } else if is_proc_still_alive(event.pid) {
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
    async fn evaluate_app(&self, pid: u32, comm: &str) -> Option<(bool, PidType, u32)> {
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

        if check_steam_environ(&environ) || check_gpu_env(&environ) {
            return Some((true, PidType::Allowed, 0));
        }

        // Check the database now, we can take our time since if we reached it, the app would've
        // been blocked
        let lookup_name = comm.to_lowercase();
        {
            let db = self.db_cache.read().await;
            if let Some(policy) = db.get(&lookup_name) {
                // For now this only work for the smart mode, will need to find a way to get the GPU
                // id to make it compatible with manual mode
                match policy {
                    GpuPolicy::Allowed => return Some((true, PidType::Allowed, 0)),
                    GpuPolicy::Blocked => return None,
                    GpuPolicy::Forced => return Some((true, PidType::Forced, 1)),
                }
            }
        }

        {
            let xdg_list = self.xdg_list.read().await;
            if let Some(meta) = xdg_list.get(&lookup_name) {
                let meta = meta.clone();
                drop(xdg_list);
                let res = self.db_tx.send((lookup_name.clone(), meta)).await;
                match res {
                    Ok(_) => info!(
                        "Discovered a new app: {}, adding to the database",
                        lookup_name
                    ),
                    Err(err) => {
                        error!("Couln't send new app to DB rw: {}", err)
                    }
                }
            }
        }

        None
    }

    #[allow(dead_code)]
    pub fn xdg_list(&self) -> Arc<RwLock<HashMap<String, AppMetadata>>> {
        Arc::clone(&self.xdg_list)
    }
}

/// Record a blocked process in the report history and notify listeners
async fn report_blocked(
    report_vec: Arc<RwLock<VecDeque<LogEntry>>>,
    signal: Option<SignalEmitter<'static>>,
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
    if let Some(signal) = signal
        && let Err(e) = LoggerInterfaceSignals::process_blocked_changed(&signal, log_entry).await
    {
        error!("failed to emit process_blocked_changed: {}", e);
    }
}

fn get_real_process_name(pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline_bytes = match fs::read(&cmdline_path) {
        Ok(b) => b,
        Err(_) => return None, // process died
    };
    if cmdline_bytes.is_empty() {
        return None;
    }
    let args: Vec<&str> = cmdline_bytes
        .split(|&b| b == 0)
        .filter_map(|b| std::str::from_utf8(b).ok())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        return None;
    }
    let binary = args[0];

    // Check Wine/Proton
    if binary.contains("wine") || binary.contains("proton") {
        for arg in args.iter().skip(1) {
            if arg.to_lowercase().ends_with(".exe") {
                let file_name = arg.split(&['/', '\\'][..]).next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }

    // Minecraft/Java games, return java instead of the real name to allow Close event bypass
    if binary.ends_with(".java") {
        for arg in args.iter().skip(1) {
            if arg.ends_with(".jar") {
                let file_name = arg.split('/').next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }
    // Fallback, just use the binary name
    let base_name = binary.split('/').next_back().unwrap_or(binary);
    Some(base_name.to_string())
}

fn is_proc_still_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Decode the 16-byte kernel comm into a String, trimming trailing NULs
fn comm_to_string(comm: [u8; 16]) -> String {
    match String::from_utf8(comm.to_vec()) {
        Ok(str) => str.trim_end_matches('\0').to_string(),
        Err(_) => "no_comm_err".to_string(),
    }
}

// TESTS

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_get_real_process_name_returns_exe_for_wine_proton_cmdline() {
        let cmdline_bytes =
            r"S:\common\NieR·Replicant·ver.1.22474487139\NieR·Replicant·ver.1.22474487139.exe"
                .as_bytes();

        assert!(!cmdline_bytes.is_empty());
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(!args.is_empty());
        {
            let binary = args[0];

            // Check Wine/Proton
            if binary.contains("wine") || binary.contains("proton") {
                for arg in args.iter().skip(1) {
                    if arg.to_lowercase().ends_with(".exe") {
                        let file_name = arg.split(&['/', '\\'][..]).next_back().unwrap_or(arg);
                        assert_eq!(file_name, "NieR·Replicant·ver.1.22474487139.exe");
                    }
                }
            }
        }
    }

    #[test]
    fn test_get_real_process_name_returns_jar_for_java_cmdline() {
        let cmdline_bytes = "minecraft.jar".as_bytes();

        assert!(!cmdline_bytes.is_empty());
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(!args.is_empty());
        {
            let binary = args[0];

            // Check Wine/Proton
            if binary.ends_with(".java") {
                for arg in args.iter().skip(1) {
                    if arg.ends_with(".jar") {
                        let file_name = arg.split('/').next_back().unwrap_or(arg);
                        assert_eq!(file_name, "minecraft");
                    }
                }
            }
        }
    }

    #[test]
    fn test_get_real_process_name_returns_basename_for_regular_binary() {
        // Simulate a regular binary like "/usr/bin/steam"
        let cmdline_bytes = b"/usr/bin/steam\0--no-browser\0";
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(!args.is_empty());
        let binary = args[0];
        let base_name = binary.split('/').next_back().unwrap_or(binary);
        assert_eq!(base_name, "steam");
    }

    #[test]
    fn test_get_real_process_name_returns_none_for_empty_cmdline() {
        let cmdline_bytes = b"";
        assert!(cmdline_bytes.is_empty());
    }

    #[test]
    fn test_get_real_process_name_extracts_wine_exe_from_multiarg_cmdline() {
        // Simulates: wine64-preloader\0C:\game\app.exe\0--fullscreen
        let cmdline_bytes = b"wine64-preloader\0C:\\game\\app.exe\0--fullscreen";
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        let binary = args[0];
        assert!(binary.contains("wine"));
        // Find the .exe argument
        let exe_arg = args
            .iter()
            .skip(1)
            .find(|a| a.to_lowercase().ends_with(".exe"));
        assert!(exe_arg.is_some());
        let file_name = exe_arg
            .unwrap()
            .split(&['/', '\\'][..])
            .next_back()
            .unwrap();
        assert_eq!(file_name, "app.exe");
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

    // ── comm_to_string ───────────────────────────────────────────────

    #[test]
    fn test_comm_to_string_trims_trailing_nuls() {
        let comm = *b"bash\0\0\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(comm_to_string(comm), "bash");
    }

    #[test]
    fn test_comm_to_string_full_length() {
        let comm = *b"a-very-long-comm";
        assert_eq!(comm_to_string(comm), "a-very-long-comm");
    }

    #[test]
    fn test_comm_to_string_invalid_utf8() {
        let comm = [0xFFu8; 16];
        assert_eq!(comm_to_string(comm), "no_comm_err");
    }

    // ── is_proc_still_alive ──────────────────────────────────────────

    #[test]
    fn test_is_proc_still_alive() {
        assert!(is_proc_still_alive(std::process::id()));
        assert!(!is_proc_still_alive(0));
    }
}
