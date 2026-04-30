use std::collections::BTreeMap;

use crate::models::{Daemon, Modes};
use cardwire_core::{
    gpu::{DbusGpuDevice, block_gpu, is_gpu_blocked}, pci::DbusPciDevice
};
use log::{error, info, warn};
use zbus::{fdo, interface};

#[interface(name = "com.github.opengamingcollective.cardwire")]
impl Daemon {
    /*
        Set the mode
    */
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: String) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::parse(&mode)?;
        // Get current_config lock
        let mut current_mode = self.state.mode_state.write().await;

        let mut blocker = self.state.ebpf_blocker.write().await;

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
                for gpu in self.state.gpu_list.values() {
                    if !gpu.is_default() {
                        block_gpu(&mut blocker, gpu, mode == Modes::Integrated)
                            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
                    };
                }
            }
            // If the auto apply is false, return all gpus to unblocked
            // Else apply the gpu_state but still unblock other gpus
            Modes::Manual => {
                let config = self.state.config.read().await;
                let gpu_state = self.state.gpu_state.read().await;
                for gpu in self.state.gpu_list.values() {
                    if gpu_state.gpu_block_state(&gpu.pci) && config.auto_apply_gpu_state() {
                        if gpu.is_default() {
                            error!(
                                "cannot set block state for GPU {}: device is marked as default",
                                gpu.id
                            );
                            // For safety, unblock if default
                            block_gpu(&mut blocker, gpu, false)
                                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
                        } else {
                            block_gpu(&mut blocker, gpu, true)
                                .map_err(|e| fdo::Error::Failed(e.to_string()))?;
                        }
                    } else {
                        block_gpu(&mut blocker, gpu, false)
                            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
                    }
                }
            }
        }
        if let Err(e) = current_mode.save_state(mode).await {
            warn!("mode couldn't be saved to config: {e}");
        }
        info!("Switched to {}", mode);
        Ok(())
    }
    #[zbus(property)]
    pub(crate) async fn mode(&self) -> fdo::Result<String> {
        let current_mode = self.state.mode_state.read().await;
        Ok(current_mode.mode().to_string())
    }

    pub(crate) async fn set_gpu_block(&self, gpu_id: u32, block: bool) -> fdo::Result<()> {
        let mut blocker = self.state.ebpf_blocker.write().await;
        let mut gpu_state = self.state.gpu_state.write().await;
        let gpu = self
            .state
            .gpu_list
            .get(&(gpu_id as usize))
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("Unknown GPU id: {}", gpu_id)))?;

        // prevent default gpu from being blocked
        if gpu.is_default() {
            error!(
                "cannot set block state for GPU {}: device is marked as default",
                gpu_id
            );
            // for safety, unblock if default & save
            block_gpu(&mut blocker, gpu, false).map_err(|e| fdo::Error::Failed(e.to_string()))?;
            if let Err(e) = gpu_state.save_state(&self.state.gpu_list, &blocker).await {
                warn!("could not save gpu_state to file: {e}");
            }
            return Err(fdo::Error::AccessDenied(format!(
                "GPU {} is the default device and cannot be blocked",
                gpu_id
            )));
        }

        block_gpu(&mut blocker, gpu, block).map_err(|err| fdo::Error::Failed(err.to_string()))?;

        info!("Set GPU {} ({}) block={}", gpu_id, gpu.pci_address(), block);
        if let Err(e) = gpu_state.save_state(&self.state.gpu_list, &blocker).await {
            warn!("could not save gpu_state to file: {e}");
        }
        Ok(())
    }

    pub(crate) async fn list_devices(&self) -> fdo::Result<BTreeMap<usize, DbusGpuDevice>> {
        let blocker = self.state.ebpf_blocker.read().await;
        let list = self.state.gpu_list.clone();
        let mut dbus_list: BTreeMap<usize, DbusGpuDevice> = BTreeMap::new();
        for (id, gpu) in list {
            let temp_gpu = DbusGpuDevice {
                id: gpu.id,
                pci: gpu.pci.clone(),
                render: gpu.render,
                name: gpu.name.clone(),
                card: gpu.card,
                default: gpu.default.unwrap_or(false),
                blocked: is_gpu_blocked(&blocker, &gpu).unwrap_or(false),
                nvidia: gpu.is_nvidia(),
                nvidia_minor: if gpu.nvidia_minor().is_some() {
                    gpu.nvidia_minor().unwrap().to_string()
                } else {
                    "".to_string()
                },
            };
            dbus_list.insert(id, temp_gpu);
        }
        Ok(dbus_list)
    }

    pub(crate) async fn list_devices_pci(&self) -> fdo::Result<BTreeMap<String, DbusPciDevice>> {
        let pci_list = &self.state.pci_devices;
        let mut dbus_list: BTreeMap<String, DbusPciDevice> = BTreeMap::new();
        for (id, pci) in pci_list {
            let temp_pci = DbusPciDevice {
                pci_address: pci.pci_address.clone(),
                iommu_group: if let Some(iommu) = pci.iommu_group {
                    iommu.to_string()
                } else {
                    "".to_string()
                },
                vendor_id: pci.vendor_id.clone().unwrap_or("".to_string()),
                device_id: pci.device_id.clone().unwrap_or("".to_string()),
                vendor_name: pci.vendor_name.clone().unwrap_or("".to_string()),
                device_name: pci.device_name.clone().unwrap_or("".to_string()),
                driver: pci.driver.clone().unwrap_or("".to_string()),
                class: pci.class.clone().unwrap_or("".to_string()),
            };
            dbus_list.insert(id.clone(), temp_pci);
        }

        Ok(dbus_list)
    }
}
