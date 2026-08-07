use aya::maps::{HashMap as AyaHashMap, MapError as AyaMapError};
use cardwire_ebpf_userspace::EbpfBlocker;
use std::{collections::HashMap, path::Path, sync::Arc};

use tokio::sync::{Mutex, RwLock};
use zbus::{
    fdo::{self, Error::Failed}, interface
};

use crate::file::{CardwireDatabase, DbusAppMetadata, GpuPolicy};

#[derive(Clone, Debug)]
pub struct SmartPolicyInterface {
    pid_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
    forced_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
    pub database: CardwireDatabase,
    policy_lock: Arc<Mutex<()>>,
}

impl SmartPolicyInterface {
    pub fn build(blocker: &mut EbpfBlocker, db: CardwireDatabase) -> Self {
        let pid_map = Arc::clone(&blocker.pid_map);
        let forced_map = Arc::clone(&blocker.forced_map);

        Self {
            pid_map,
            forced_map,
            database: db,
            policy_lock: Arc::new(Mutex::new(())),
        }
    }
}

#[interface(name = "org.opengamingcollective.cardwire.SmartPolicy")]
impl SmartPolicyInterface {
    /// Authorized a pid to access a specific GPU
    pub async fn request_process_access(
        &self,
        pid: u32,
        policy: String,
        value: u32,
    ) -> Result<(), fdo::Error> {
        // Check if the process exists, leave if it doesnt
        if !Path::new(&format!("/proc/{}", pid)).exists() {
            return Err(fdo::Error::Failed("process doesn't exist".to_string()));
        }
        // Match the policy and add the pid to the corresponding ebpf map
        match policy.as_str() {
            // Default, do nothing
            "Default" => Ok(()),
            // Equivalent to CARDWIRE_ALLOW=1, show both iGPU and dGPU
            "Allow_dGPU" => {
                {
                    // First remove the PID from the other map if present
                    let mut forced_map = self.forced_map.write().await;
                    forced_map
                        .remove(&pid)
                        .map_err(|err| fdo::Error::Failed(err.to_string()))?;
                }
                let mut pid_map = self.pid_map.write().await;
                pid_map
                    .insert(pid, 0, 0)
                    .map_err(|err| fdo::Error::Failed(err.to_string()))
            }
            // Equivalent to CARDWIRE_FORCE_DGPU=value
            "Force_dGPU" => {
                {
                    // First remove the PID from the other map if present
                    let mut pid_map = self.pid_map.write().await;
                    pid_map
                        .remove(&pid)
                        .map_err(|err| fdo::Error::Failed(err.to_string()))?;
                }
                let mut force_map = self.forced_map.write().await;
                force_map
                    .insert(pid, value, 0)
                    .map_err(|err| Failed(err.to_string()))
            }
            // Equivalent to CARDWIRE_FORCE_GPU=value
            "Force_GPU" => {
                {
                    // First remove the PID from the other map if present
                    let mut pid_map = self.pid_map.write().await;
                    pid_map
                        .remove(&pid)
                        .map_err(|err| fdo::Error::Failed(err.to_string()))?;
                }
                let mut force_map = self.forced_map.write().await;
                force_map
                    .insert(pid, value, 0)
                    .map_err(|err| Failed(err.to_string()))
            }
            _ => Err(fdo::Error::InvalidArgs(format!("invalid arg: {}", policy))),
        }
    }

    /// Check if the process is inside PID or FORCED map, and return the map type with the gpu_id
    /// associed
    pub async fn get_process_status(&self, pid: u32) -> Result<(String, Option<u32>), fdo::Error> {
        let mut status = String::new();
        let mut gpu_id: Option<u32> = None;

        {
            let pid_map = self.pid_map.read().await;
            match pid_map.get(&pid, 0) {
                Ok(id) => {
                    status = "Allowed".to_string();
                    gpu_id = Some(id)
                }
                Err(err) => match err {
                    AyaMapError::KeyNotFound => {}
                    _ => {
                        return Err(fdo::Error::Failed(format!(
                            "Couldn't read PID MAP: {}",
                            err
                        )));
                    }
                },
            }
        }
        {
            let forced_map = self.forced_map.read().await;
            match forced_map.get(&pid, 0) {
                Ok(id) => {
                    status = "Forced".to_string();
                    gpu_id = Some(id)
                }
                Err(err) => match err {
                    AyaMapError::KeyNotFound => {}
                    _ => {
                        return Err(fdo::Error::Failed(format!(
                            "Couldn't read FORCED MAP: {}",
                            err
                        )));
                    }
                },
            }
        }

        Ok((status, gpu_id))
    }

    pub async fn get_app_policies(&self) -> fdo::Result<HashMap<String, DbusAppMetadata>> {
        let db_clone = self.database.clone();

        tokio::task::spawn_blocking(move || {
            db_clone
                .read_db()
                .map_err(|err| fdo::Error::Failed(err.to_string()))
        })
        .await
        .map_err(|err| fdo::Error::Failed(err.to_string()))?
    }

    pub async fn set_app_policies(&self, app_id: String, policy: i32) -> Result<(), fdo::Error> {
        let gpu_policy = GpuPolicy::try_from_i32(policy)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("invalid policy: {}", policy)))?;

        if !self.database.cache.read().await.contains_key(&app_id) {
            return Err(fdo::Error::UnknownObject(format!(
                "app not found: {}",
                app_id
            )));
        }

        let db_clone = self.database.clone();
        let app_id_clone = app_id.clone();

        let _policy_guard = self.policy_lock.lock().await;
        tokio::task::spawn_blocking(move || db_clone.update_policy(&app_id_clone, policy))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        self.database.cache.write().await.insert(app_id, gpu_policy);

        Ok(())
    }
}
