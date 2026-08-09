//! Define the mode dbus
use crate::{
    core::gpu::{restart_nvidia_powerd, send_drm_uevent}, file::{CardwireGpuState, CardwireModeState}, interface::{DaemonContext, GpuInterface, config::ConfigMemory}, types::SystemType
};
use anyhow::Result;
use aya::maps::Array as AyaArray;
use log::{error, info, warn};
use std::{
    collections::BTreeMap, sync::{Arc, OnceLock, atomic::Ordering}
};
use tokio::{
    sync::{Mutex, RwLock}, task
};
use zbus::{fdo, interface, object_server::SignalEmitter};

pub use crate::types::Modes;

#[derive(Clone)]
pub struct ModeInterface {
    mode_state: Arc<RwLock<CardwireModeState>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
    gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
    config: Arc<ConfigMemory>,
    mode_map: Arc<Mutex<AyaArray<aya::maps::MapData, u8>>>,
    // Mutex to serialize mode transitions
    transition: Arc<Mutex<()>>,
    // Signal emitter for mode changes (used by background tasks), populated once the interface is
    // served
    pub signal_emitter: Arc<OnceLock<SignalEmitter<'static>>>,
}

impl ModeInterface {
    pub async fn build(context: &DaemonContext) -> Result<ModeInterface> {
        let mut blocker = context.blocker.write().await;
        let mode_map: aya::maps::Array<aya::maps::MapData, u8> = blocker.get_mode_map()?;
        let mode_map = Arc::new(Mutex::new(mode_map));
        Ok(ModeInterface {
            mode_state: context.mode_state.clone(),
            gpu_state: context.gpu_state.clone(),
            gpu_list: context.gpu_list.clone(),
            config: context.config.clone(),
            mode_map,
            transition: Arc::new(Mutex::new(())),
            signal_emitter: Arc::new(OnceLock::new()),
        })
    }

    /// set the mode in the `cardwire_mode` bpf map
    async fn update_mode_bpf_map(&self, mode: Modes) -> fdo::Result<()> {
        let mut mode_map = self.mode_map.lock().await;
        let mode: u32 = Modes::into(mode);
        mode_map
            .set(0, mode as u8, 0)
            .map_err(|err| fdo::Error::Failed(err.to_string()))
    }

    /// Apply a mode and optionally persist it to the state file
    pub async fn internal_set_mode(&self, mode: Modes, save: bool) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        self.apply_mode(mode).await?;
        // Save
        {
            let mut state = self.mode_state.write().await;
            if let Err(e) = state.save_state(mode, save).await {
                warn!("mode couldn't be saved to config: {e}");
            }
        }

        if let Some(emitter) = self.signal_emitter.get()
            && let Err(err) = self.mode_changed(emitter).await
        {
            warn!("failed to emit mode change signal: {err}");
        };

        // Emit block_changed signal after the mode has been applied and send drm uevent
        let gpu_list = self.gpu_list.read().await;
        for gpu in gpu_list.values().filter(|gpu| gpu.device.is_available()) {
            let Some(emitter) = gpu.signal_emitter.get() else {
                continue;
            };
            if let Err(err) = gpu.block_changed(emitter).await {
                warn!(
                    "failed to emit Block property change for {}: {err}",
                    gpu.device.name()
                );
            }
            if let Err(err) = send_drm_uevent(gpu.id).await {
                warn!("failed to send drm uevent for {}: {err}", gpu.device.name());
            };
        }

        // and at last, try to restart nvidia-powerd, ignore if error, it will be logged
        task::spawn(restart_nvidia_powerd());

        Ok(())
    }

    /// Apply a mode to GPU blocking and the eBPF map without persisting it
    pub(crate) async fn apply_mode(&self, mode: Modes) -> fdo::Result<()> {
        let gpu_list = self.gpu_list.read().await;
        let system_type = SystemType::from_gpulist(&gpu_list);

        match mode {
            // Integrated and Smart modes only work on hybrid setups with a offload discrete GPU
            // (laptops)
            Modes::Integrated | Modes::Smart => {
                // Check if there is an offload discrete GPU (discrete and not the default display)
                if system_type != SystemType::Laptop {
                    let error_message = format!(
                        "Couldn't set mode to {}, Integrated and Smart modes require a offload discrete GPU (not supported on desktops where the discrete GPU is the primary display)",
                        mode
                    );
                    error!("{}", error_message);
                    return Err(fdo::Error::NotSupported(error_message));
                }

                for (id, gpu) in gpu_list.iter().filter(|(_, gpu)| gpu.device.is_available()) {
                    if gpu.device.is_discrete() && !gpu.device.is_default() {
                        // Here we block the offload dGPU
                        gpu.block_gpu(*id as u32).await?;
                    } else if mode == Modes::Smart
                        && gpu.device.is_default()
                        && !gpu.device.is_discrete()
                    {
                        // push default gpu (iGPU) into the blocked inode map for tracking only
                        gpu.block_gpu(*id as u32).await?;
                    }
                }
            }

            // Hybrid mode unblocks all GPUs so all are available to the system
            Modes::Hybrid => {
                for gpu in gpu_list.values().filter(|gpu| gpu.device.is_available()) {
                    gpu.unblock_gpu().await?;
                }
            }

            // If the auto apply is false, return all gpus to unblocked
            // Else apply the gpu_state but still unblock other gpus
            Modes::Manual => {
                // Manual is only allowed on Desktop or Manual
                if system_type != SystemType::Manual && system_type != SystemType::Desktop {
                    let error_message = format!(
                        "Couldn't set mode to {}, Manual mode is only available on Desktop or system with either 1 GPU or 3+ GPUs",
                        mode
                    );
                    error!("{}", error_message);
                    return Err(fdo::Error::NotSupported(error_message));
                }
                let config = self.config.auto_apply_gpu_state.load(Ordering::Relaxed);
                let gpu_state = self.gpu_state.read().await;
                for (id, gpu) in gpu_list.iter().filter(|(_, gpu)| gpu.device.is_available()) {
                    if gpu_state.gpu_block_state(gpu.device.pci().pci_address()) && config {
                        if gpu.device.is_default() {
                            // For safety, warn and unblock if default
                            warn!(
                                "auto_apply_gpu_state tried to block gpu: {}, which is the default gpu, unblocking for safety...",
                                gpu.device.name()
                            );
                            gpu.unblock_gpu().await?;
                        } else {
                            info!("blocking: {} ", gpu.device.pci().pci_address());
                            gpu.block_gpu(*id as u32).await?;
                        }
                    } else {
                        gpu.unblock_gpu().await?;
                    }
                }
            }
        }

        // Now update the hashmap value to let the bpf know the new mode
        self.update_mode_bpf_map(mode).await?;

        info!("Switched to {}", mode);
        Ok(())
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Mode")]
impl ModeInterface {
    /// Set the requested mode
    #[zbus(property)]
    pub async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        self.internal_set_mode(mode, true).await?;
        Ok(())
    }

    /// Return the mode currently applied
    #[zbus(property)]
    pub async fn mode(&self) -> fdo::Result<u32> {
        Ok(u32::from(self.mode_state.read().await.mode()))
    }

    // zbus method
    /// List of available modes depending of the system type
    pub async fn available_modes(&self) -> fdo::Result<Vec<Modes>> {
        let system_type = {
            let gpu_list = self.gpu_list.read().await;

            SystemType::from_gpulist(&gpu_list)
        };
        Ok(match system_type {
            SystemType::Laptop => {
                vec![Modes::Integrated, Modes::Hybrid, Modes::Smart]
            }
            SystemType::Desktop | SystemType::Manual => {
                vec![Modes::Hybrid, Modes::Manual]
            }
        })
    }
}
