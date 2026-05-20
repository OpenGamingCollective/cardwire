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
        let mut gpu_list = self.gpu_list.write().await;
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
                    let mut state = gpu.inner.write().await;
                    if !state.device.is_default() {
                        if mode == Modes::Integrated {
                            state.block_gpu().await?;
                        } else {
                            state.unblock_gpu().await?;
                        }
                    };
                }
            }
            // If the auto apply is false, return all gpus to unblocked
            // Else apply the gpu_state but still unblock other gpus
            Modes::Manual => {
                //let gpu_state = self.state.gpu_state.read().await;
                for (_, gpu) in gpu_list.iter_mut() {
                    let mut state = gpu.inner.write().await;
                    state.unblock_gpu().await?;
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
        let gpu_list = self.mode_state.gpu_list.read().await;
        let mut dbus_list: BTreeMap<usize, DbusGpuDevice> = BTreeMap::new();
        for (id, gpu) in gpu_list.iter() {
            let state = gpu.inner.write().await;
            let temp_gpu = DbusGpuDevice {
                id: *id as u32,
                pci: state.device.pci.pci_address().to_string(),
                render: *state.device.render(),
                name: state.device.name().to_string(),
                card: *state.device.card(),
                default: state.device.default().unwrap_or(false),
                blocked: state.blocked(),
                nvidia: state.device.nvidia(),
                nvidia_minor: if state.device.nvidia_minor().is_some() {
                    state.device.nvidia_minor().unwrap().to_string()
                } else {
                    "".to_string()
                },
            };
            dbus_list.insert(*id, temp_gpu);
        }
        Ok(dbus_list)
    }
}
