use crate::models::{Daemon, Modes};
use cardwire_core::gpu::{GpuRow, block_gpu, is_gpu_blocked};
use log::{error, info, warn};
use zbus::{fdo, interface};

#[interface(name = "com.github.luytan.cardwire")]
impl Daemon {
    /*
        Set the mode
    */
    pub(crate) async fn set_mode(&self, mode: String) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::parse(&mode)?;
        // Get current_config lock
        let mut current_config = self.state.config.write().await;

        match mode {
            // Integrated/Hybrid only works on laptop with two gpus, will refuse if the computer has
            // more than 2 gpus
            Modes::Integrated | Modes::Hybrid => {
                if self.state.gpu_list.len() != 2 {
                    error!(
                        "Couldn't set mode to {}, the mode require exactly 2 GPUs",
                        mode
                    );
                    return Err(fdo::Error::NotSupported(format!(
                        "Couldn't set mode to {}, the mode require exactly 2 GPUs",
                        mode
                    )));
                }
                // Loop to find the non default gpu and block it,
                let mut blocker = self.state.ebpf_blocker.write().await;
                for gpu in self.state.gpu_list.values() {
                    if !gpu.is_default() {
                        block_gpu(&mut blocker, gpu, mode == Modes::Integrated)
                            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
                    };
                }
            }
            // Mode manual should return all gpus to a non blocked state and allow gpu <id> block
            // on/off
            Modes::Manual => {}
        }

        current_config.mode = mode;

        if let Err(err) = current_config.save_config() {
            warn!("Failed to save config: {}", err);
        }
        info!("Switched to {}", mode);
        Ok(())
    }

    pub(crate) async fn get_mode(&self) -> String {
        format!("Current Mode: {}", self.state.mode().await)
    }

    pub(crate) async fn set_gpu_block(&self, gpu_id: u32, block: bool) -> fdo::Result<()> {
        let gpu = self
            .state
            .gpu_list
            .get(&(gpu_id as usize))
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("Unknown GPU id: {}", gpu_id)))?;

        // prevent default gpu from being blocked
        if gpu.is_default() {
            error!(
                "Cannot set block state for GPU {}: device is marked as default",
                gpu_id
            );
            return Err(fdo::Error::AccessDenied(format!(
                "GPU {} is the default device and cannot be blocked",
                gpu_id
            )));
        }

        let mut blocker = self.state.ebpf_blocker.write().await;
        block_gpu(&mut blocker, gpu, block).map_err(|err| fdo::Error::Failed(err.to_string()))?;

        info!("Set GPU {} ({}) block={}", gpu_id, gpu.pci_address(), block);

        Ok(())
    }

    pub(crate) async fn list_gpus(&self) -> Vec<GpuRow> {
        //self.list_gpu_rows().await
        let mut rows = Vec::with_capacity(self.state.gpu_list.len());
        let blocker = self.state.ebpf_blocker.read().await;
        for gpu in self.state.gpu_list.values() {
            let blocked: bool = match is_gpu_blocked(&blocker, gpu) {
                Ok(b) => b,
                Err(e) => {
                    error!(
                        "Couldn't check gpu's lock state for {}: {}",
                        gpu.pci_address(),
                        e
                    );
                    false
                }
            };
            rows.push((
                gpu.id(),
                gpu.name().to_string(),
                gpu.pci_address().to_string(),
                gpu.render_node().to_string(),
                gpu.is_default(),
                blocked,
            ));
        }
        rows.sort_by_key(|row| row.0);
        rows
    }
}
