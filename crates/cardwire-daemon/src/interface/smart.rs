use aya::maps::HashMap as AyaHashMap;
use cardwire_ebpf_userspace::EbpfBlocker;
use std::{path::Path, sync::Arc};

use tokio::sync::RwLock;
use zbus::{
    fdo::{
        self, Error::{Failed, InvalidArgs}
    }, interface
};

#[derive(Clone, Debug)]
pub struct SmartPolicyInterface {
    pid_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
    forced_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u32>>>,
}

impl SmartPolicyInterface {
    pub fn build(blocker: &mut EbpfBlocker) -> Self {
        let pid_map = Arc::clone(&blocker.pid_map);
        let forced_map = Arc::clone(&blocker.forced_map);

        Self {
            pid_map,
            forced_map,
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
                let mut pid_map = self.pid_map.write().await;
                let _res = pid_map
                    .insert(pid, 0, 0)
                    .map_err(|err| fdo::Error::Failed(err.to_string()))?;
                Ok(())
            }
            // Equivalent to CARDWIRE_FORCE_DGPU=value
            "Force_dGPU" => {
                let mut force_map = self.forced_map.write().await;
                force_map
                    .insert(pid, value, 0)
                    .map_err(|err| Failed(err.to_string()))
            }
            // Equivalent to CARDWIRE_FORCE_GPU=value
            "Force_GPU" => {
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
            if let Ok(gpu) = pid_map.get(&pid, 0) {
                status = "Allowed".to_string();
                gpu_id = Some(gpu)
            }
        }
        {
            let forced_map = self.forced_map.read().await;
            if let Ok(gpu) = forced_map.get(&pid, 0) {
                status = "Forced".to_string();
                gpu_id = Some(gpu)
            }
        }

        Ok((status, gpu_id))
    }
}
