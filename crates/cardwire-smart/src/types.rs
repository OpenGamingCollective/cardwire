use std::{
    collections::{HashMap, HashSet}, sync::Arc
};

use aya::maps::{HashMap as AyaHashMap, RingBuf};
use cardwire_analyzer::{
    dynamic::{check_env, get_steam_app_id, read_process_environ}, helpers::{get_real_process_name, strip_nix_wrap}, r#static::{get_fdo_apps, watch_fdo_folders}
};
use cardwire_core::app_metadata::types::AppMetadata;
use cardwire_ebpf_userspace::{EbpfBlocker, types::ExecEvent};
use log::{error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock, mpsc, oneshot}, task
};

use crate::errors::{Result, SmartAnalyzerError};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuPolicy {
    Blocked = 0,
    Allowed = 1,
    Forced = 2,
}
impl GpuPolicy {
    pub fn from_i32(val: i32) -> Self {
        match val {
            0 => GpuPolicy::Blocked,
            1 => GpuPolicy::Allowed,
            2 => GpuPolicy::Forced,
            _ => GpuPolicy::Blocked,
        }
    }

    #[allow(unused)]
    pub fn try_from_i32(val: i32) -> Option<Self> {
        match val {
            0 => Some(GpuPolicy::Blocked),
            1 => Some(GpuPolicy::Allowed),
            2 => Some(GpuPolicy::Forced),
            _ => None,
        }
    }
}

// Tried to keep it as readable as possible

pub struct SmartAnalyzer {
    exec_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    bpf_maps: Maps,
    xdg_entries: Xdg,
    database: Database,
}

struct Maps {
    allowed_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
    forced_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
}

struct Xdg {
    xdg_list: Arc<RwLock<HashMap<String, AppMetadata>>>,
    xdg_folders: Vec<std::path::PathBuf>,
}

pub struct Database {
    db_cache: Arc<RwLock<HashMap<String, i32>>>,
    pending_discoveries: Arc<Mutex<HashSet<String>>>,
    db_tx: mpsc::Sender<(String, AppMetadata, oneshot::Sender<bool>)>,
}

impl SmartAnalyzer {
    // TODO: Reduce the number of args
    pub async fn build(
        blocker: Arc<RwLock<EbpfBlocker>>,
        db_cache: Arc<RwLock<HashMap<String, i32>>>,
        db_tx: mpsc::Sender<(String, AppMetadata, oneshot::Sender<bool>)>,
    ) -> Result<Self> {
        let mut blocker = blocker.write().await;

        let exec_ring = blocker.get_exec_ring()?;
        let exec_ring = AsyncFd::new(exec_ring)?;
        let exec_ring = Arc::new(Mutex::new(exec_ring));

        let maps = Maps {
            allowed_map: blocker.pid_map.clone(),
            forced_map: blocker.forced_map.clone(),
        };

        let xdg_res = get_fdo_apps().await?;

        let xdg = Xdg {
            xdg_list: Arc::new(RwLock::new(xdg_res.0)),
            xdg_folders: xdg_res.1,
        };

        let database = Database {
            db_cache,
            pending_discoveries: Arc::default(),
            db_tx,
        };
        Ok(Self {
            exec_ring,
            bpf_maps: maps,
            xdg_entries: xdg,
            database,
        })
    }
    pub async fn run(self) -> Result<()> {
        // Clone the arc to move into the async task
        let exec_arc = self.exec_ring.clone();

        let shared_self = Arc::new(self);

        let cloned_xdg_folders = shared_self.xdg_entries.xdg_folders.clone();
        let cloned_xdg_list = shared_self.xdg_entries.xdg_list.clone();

        // Task that watch the xdg folders for new desktop entries
        task::spawn(async move { watch_fdo_folders(cloned_xdg_folders, cloned_xdg_list) });

        // smart task is the only one locking it, it shouldn't dead lock
        let mut exec_ring = exec_arc.lock().await;

        loop {
            if let Ok(mut guard) = exec_ring.ready_mut(Interest::READABLE).await
                && guard.ready().is_readable()
            {
                while let Some(item) = guard.get_inner_mut().next() {
                    match ExecEvent::from_item(item) {
                        Some(event) => {
                            let this = shared_self.clone();
                            task::spawn(async move { this.analyze_event(event).await });
                        }
                        None => {
                            continue;
                        }
                    }
                }
                guard.clear_ready();
            }
        }
    }

    /// Analyze an exec event and whitelist it if allowed or not
    async fn analyze_event(&self, event: ExecEvent) -> () {
        let pid_map = self.bpf_maps.allowed_map.read().await;

        // PID was already analyzed, ignore
        // TODO: may break some apps that change their comm later ?
        if pid_map.get(&event.pid, 0).is_ok() {
            return;
        }
        drop(pid_map);

        let comm = match get_real_process_name(event.pid) {
            Some(name) => name,
            None => return,
        };

        if let Some(res) = self.evaluate_process(event.pid, &comm, event.mode).await
            && res.do_action
        {
            match res.policy {
                GpuPolicy::Blocked => {
                    // do nothing
                }
                GpuPolicy::Allowed => {
                    let mut allowed_map = self.bpf_maps.allowed_map.write().await;
                    // TODO: use the GPU id as value
                    if let Err(err) = allowed_map.insert(event.pid, 1, 0) {
                        warn!("Failed to insert into eBPF map: {}", err)
                    }
                }
                GpuPolicy::Forced => {
                    let mut forced_map = self.bpf_maps.forced_map.write().await;
                    if let Err(e) = forced_map.insert(event.pid, res.gpu_id, 0) {
                        warn!("Failed to insert into eBPF map: {}", e);
                    }
                }
            }
        }
    }

    /// Evaluate if the process should be blocked, allowed or forced
    async fn evaluate_process(&self, pid: u32, comm: &str, mode: u8) -> Option<EvaluationResult> {
        let environ = match read_process_environ(pid) {
            Ok(b) => b,
            Err(_) => return None,
        };

        // First check for cardwire envs
        if let Some(res) = get_cardwire_envs(&environ) {
            return Some(res);
        }

        // No cardwire envs

        // If manual mode, do not process app discovery or database policies
        // TODO: allow manual mode
        if mode == 2 {
            return None;
        }

        // Now use cardwire's application policy

        let mut process = comm.to_lowercase();

        // Its easier to use Steam AppId for steam games matching
        if let Some(steam_app_id) = get_steam_app_id(&environ) {
            process = steam_app_id
        }

        // Check if our process is in the db
        if let Some(res) = self.is_process_in_db(&process).await {
            return Some(res);
        }

        // App wasnt in DB, try to discover it using the xdg desktop entries

        let _ = self.discover_app(comm).await;

        None
    }

    async fn is_process_in_db(&self, comm: &str) -> Option<EvaluationResult> {
        let database = self.database.db_cache.read().await;

        if let Some(policy) = database.get(comm) {
            return Some(EvaluationResult {
                do_action: true,
                policy: GpuPolicy::from_i32(*policy),
                // hardcoded to 0 for now, forced policy in the DB is not coded yet
                // and no user frontend can set this policy
                gpu_id: 0,
            });
        }

        None
    }
    async fn discover_app(&self, comm: &str) -> Result<()> {
        let xdg_entries = self.xdg_entries.xdg_list.read().await;

        // strip the nix wrapper
        let comm = strip_nix_wrap(&comm);

        // TODO: add support for binaries that do not have a desktop entry
        if let Some(metadata) = xdg_entries.get(&comm) {
            let metadata = metadata.clone();
            // TODO: Refactor error handling in that crate
            let _ = self.insert_to_db(&comm, metadata).await;
            // app was discovered, return now
            return Ok(());
        }

        // For steam AppId
        if let Some(app_id) = comm.strip_prefix("steam_app_") {
            let metadata = AppMetadata {
                display_name: format!("Steam Game {}", app_id),
                desktop_file_id: None,
                icon_name: Some(format!("steam_app_{}", app_id)),
            };
            let _ = self.insert_to_db(&comm, metadata).await;
            return Ok(());
        }

        Ok(())
    }
    async fn insert_to_db(&self, comm: &str, metadata: AppMetadata) -> Result<()> {
        // One app at a time
        {
            let mut lock = self.database.pending_discoveries.lock().await;
            if !lock.insert(comm.to_string()) {
                // alr in the map
                return Ok(());
            }
        }

        // First add the new app to the database
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        let res = self
            .database
            .db_tx
            .send((comm.to_string(), metadata, reply_tx))
            .await;
        match res {
            Ok(_) => match reply_rx.await {
                Ok(true) => {
                    // Process successfully inserted into the db
                    info!(
                        "Discovered a new process: {}, adding to application policy",
                        comm
                    );
                }
                Ok(false) => {
                    // Duplicated or write failure, do nothing
                }
                Err(err) => {
                    let err = SmartAnalyzerError::DbReceiverError(err, comm.to_string());
                    error!("{}", err);
                    return Err(err);
                }
            },
            Err(err) => {
                let err = SmartAnalyzerError::DbSendError(err);
                error!("{}", err);
                return Err(err);
            }
        }

        // Add to the cache and default to blocked
        self.database
            .db_cache
            .write()
            .await
            .insert(comm.to_string(), GpuPolicy::Blocked as i32);

        self.database.pending_discoveries.lock().await.remove(comm);

        Ok(())
    }
}

fn get_cardwire_envs(environ: &Vec<u8>) -> Option<EvaluationResult> {
    if let Some(allow) = check_env("CARDWIRE_ALLOW", &environ) {
        return Some(EvaluationResult {
            do_action: allow == 1,
            policy: GpuPolicy::Allowed,
            gpu_id: 0,
        });
    }
    if let Some(value) = check_env("CARDWIRE_FORCE_DGPU", &environ) {
        return Some(EvaluationResult {
            do_action: value == 1,
            policy: GpuPolicy::Forced,
            gpu_id: value,
        });
    }
    if let Some(value) = check_env("CARDWIRE_FORCE_GPU", &environ) {
        return Some(EvaluationResult {
            do_action: true,
            policy: GpuPolicy::Forced,
            gpu_id: value,
        });
    }
    None
}

/// Result of a process evaluation
struct EvaluationResult {
    do_action: bool,
    policy: GpuPolicy,
    gpu_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_policy_from_i32_defaults_to_blocked() {
        assert_eq!(GpuPolicy::from_i32(-1), GpuPolicy::Blocked);
        assert_eq!(GpuPolicy::from_i32(42), GpuPolicy::Blocked);
        assert_eq!(GpuPolicy::try_from_i32(-1), None);
        assert_eq!(GpuPolicy::try_from_i32(42), None);
        assert_eq!(GpuPolicy::try_from_i32(1), Some(GpuPolicy::Allowed));
    }
}
