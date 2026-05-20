//! handle the dbus part of cardwired
use std::collections::BTreeMap;

use crate::models::{Daemon, Modes};
use cardwire_core::gpu::DbusGpuDevice;
use log::{error, info, warn};
use zbus::{fdo, interface};

#[interface(name = "com.github.opengamingcollective.cardwire")]
impl Daemon {
    /*
        Set the mode
    */
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::parse(&mode)?;
        let mut current_mode = self.mode_state.mode_config.write().await;
        let mut gpu_list = self.mode_state.gpu_list.write().await;

        match mode {
            // Integrated/Hybrid only works on laptop with two gpus, will refuse if the computer has
            // more than 2 gpus
            Modes::Integrated | Modes::Hybrid => {
                if gpu_list.len() != 2 {
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
                for gpu in gpu_list.values_mut() {
                    if !gpu.device.is_default() {
                        gpu.block_gpu().await?;
                    };
                }
            }
            // If the auto apply is false, return all gpus to unblocked
            // Else apply the gpu_state but still unblock other gpus
            Modes::Manual => {
                //let gpu_state = self.state.gpu_state.read().await;
                for (_, gpu) in gpu_list.iter_mut() {
                    gpu.unblock_gpu().await?;
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
    pub(crate) async fn mode(&self) -> fdo::Result<u32> {
        let current_mode = self.mode_state.mode_config.read().await;
        match current_mode.mode() {
            Modes::Integrated => Ok(0),
            Modes::Hybrid => Ok(1),
            Modes::Manual => Ok(2),
        }
    }

    pub(crate) async fn list_devices(&self) -> fdo::Result<BTreeMap<usize, DbusGpuDevice>> {
        let gpu_list = self.gpu_state.gpu_list.read().await;
        let mut dbus_list: BTreeMap<usize, DbusGpuDevice> = BTreeMap::new();
        for (id, gpu) in gpu_list.iter() {
            let temp_gpu = DbusGpuDevice {
                id: *id as u32,
                pci: gpu.device.pci.pci_address().to_string(),
                render: *gpu.device.render(),
                name: gpu.device.name().to_string(),
                card: *gpu.device.card(),
                default: gpu.device.default().unwrap_or(false),
                blocked: gpu.blocked(),
                nvidia: gpu.device.nvidia(),
                nvidia_minor: if gpu.device.nvidia_minor().is_some() {
                    gpu.device.nvidia_minor().unwrap().to_string()
                } else {
                    "".to_string()
                },
            };
            dbus_list.insert(*id, temp_gpu);
        }
        Ok(dbus_list)
    }

    //pub(crate) async fn list_devices_pci(&self) -> fdo::Result<BTreeMap<String, DbusPciDevice>> {
    //    let pci_list = &self.state.pci_devices;
    //    let mut dbus_list: BTreeMap<String, DbusPciDevice> = BTreeMap::new();
    //    for (id, pci) in pci_list {
    //        let temp_pci = DbusPciDevice {
    //            pci_address: pci.pci_address().to_string(),
    //            iommu_group: if let Some(iommu) = pci.iommu_group() {
    //                iommu.to_string()
    //            } else {
    //                "".to_string()
    //            },
    //            vendor_id: pci.vendor_id().clone().unwrap_or("".to_string()),
    //            device_id: pci.device_id().clone().unwrap_or("".to_string()),
    //            vendor_name: pci.vendor_name().clone().unwrap_or("".to_string()),
    //            device_name: pci.device_name().clone().unwrap_or("".to_string()),
    //            driver: pci.driver().clone().unwrap_or("".to_string()),
    //            class: pci.class().clone().unwrap_or("".to_string()),
    //            parent_pci: pci.parent_pci().clone().unwrap_or("".to_string()),
    //            child_pci: pci.child_pci().clone().unwrap_or("".to_string()),
    //        };
    //        dbus_list.insert(id.clone(), temp_pci);
    //    }
    //
    //    Ok(dbus_list)
    //}
    //
    //pub async fn get_status(&self, gpu_id: u32) -> fdo::Result<String> {
    //    let gpu = self
    //        .state
    //        .gpu_list
    //        .get(&(gpu_id as usize))
    //        .ok_or_else(|| fdo::Error::InvalidArgs(format!("Unknown GPU id: {}", gpu_id)))?;
    //    let gpu_pci = gpu.pci.pci_address();
    //    let power_state =
    //        fs::read_to_string(format!("/sys/bus/pci/devices/{gpu_pci}/power_state")).await;
    //    if let Ok(state) = power_state {
    //        Ok(state)
    //    } else {
    //        Err(fdo::Error::Failed("Couldn't read power_state".to_string()))
    //    }
    //}
}
