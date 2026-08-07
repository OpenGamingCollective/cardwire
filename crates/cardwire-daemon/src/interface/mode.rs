//! Define the mode dbus
use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{DaemonContext, GpuInterface, config::ConfigMemory}
};
use anyhow::Result;
use aya::maps::Array as AyaArray;
use log::{error, info, warn};
use std::{
    collections::BTreeMap, process::Stdio, sync::{Arc, OnceLock, atomic::Ordering}
};
use tokio::{
    process::Command, sync::{Mutex, RwLock}, task
};
use zbus::{fdo, interface, object_server::SignalEmitter};

pub use crate::types::Modes;

// to change a mode, we need the config, the mode_state, the gpu_list
#[derive(Clone)]
pub struct ModeInterface {
    mode_state: Arc<RwLock<CardwireModeState>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
    gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
    config: Arc<ConfigMemory>,
    mode_map: Arc<Mutex<AyaArray<aya::maps::MapData, u8>>>,
    // Mutex to serialize mode transitions
    transition: Arc<Mutex<()>>,
    // Signal emitter for automatic mode changes, populated once the interface is served
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

    /// restart the nvidia-powerd service using systemctl
    async fn restart_nvidia_powerd() {
        let service = "nvidia-powerd.service";

        let enabled = match Command::new("systemctl")
            .arg("is-enabled")
            .arg(service)
            .output()
            .await
        {
            Ok(output) => {
                if let Ok(output_str) = str::from_utf8(&output.stdout) {
                    output_str.contains("enabled")
                } else {
                    false
                }
            }
            Err(err) => {
                error!("error while trying to detect nvidia-powerd: {}", err);
                return;
            }
        };
        if enabled {
            match Command::new("systemctl")
                .arg("restart")
                .arg(service)
                .arg("--no-block")
                .stdout(Stdio::null())
                .status()
                .await
            {
                Ok(status) => {
                    if status.success() {
                        info!("successfully restart nvidia-powerd.service");
                    } else {
                        warn!("error restarting nvidia-powerd: {:?}", status.code())
                    }
                }
                Err(err) => {
                    error!("error while trying to restart nvidia-powerd: {}", err)
                }
            };
        }
    }

    /// Apply a mode and optionally persist it to the state file.
    /// - `None` or `Some(true)`: persist the mode (default)
    /// - `Some(false)`: apply only (for display override / hotplug recovery)
    pub async fn internal_set_mode(&self, mode: Modes, save: Option<bool>) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        self.apply_mode(mode).await?;
        let mut state = self.mode_state.write().await;
        if let Err(e) = state.save_state(mode, save.unwrap_or(true)).await {
            warn!("mode couldn't be saved to config: {e}");
        }
        if let Some(emitter) = self.signal_emitter.get() {
            self.mode_changed(emitter).await?;
        }
        Ok(())
    }

    /// Apply a mode to GPU blocking and the eBPF map without persisting it.
    ///
    /// Keeping persistence out of this helper lets display overrides restore the requested mode.
    pub(crate) async fn apply_mode(&self, mode: Modes) -> fdo::Result<()> {
        let gpu_list = self.gpu_list.read().await;
        match mode {
            // Integrated and Smart modes only work on hybrid setups with a offload discrete GPU
            // (laptops)
            Modes::Integrated | Modes::Smart => {
                let available: Vec<(u32, bool, bool, u32)> = gpu_list
                    .iter()
                    .filter(|(_, gpu)| gpu.device.is_available())
                    .map(|(id, gpu)| {
                        (
                            *id as u32,
                            gpu.device.is_discrete(),
                            gpu.device.is_default(),
                            *gpu.device.card(),
                        )
                    })
                    .collect();

                if available.len() != 2 {
                    let error_message = format!(
                        "Couldn't set mode to {}, the mode requires exactly 2 GPUs",
                        mode
                    );
                    error!("{}", error_message);
                    return Err(fdo::Error::NotSupported(error_message));
                }

                // Check if there is an offload discrete GPU (discrete and not the default display)
                let has_offload_dgpu = available
                    .iter()
                    .any(|(_, discrete, default, _)| *discrete && !*default)
                    && available
                        .iter()
                        .any(|(_, discrete, default, _)| !*discrete && *default);

                if !has_offload_dgpu {
                    let error_message = format!(
                        "Couldn't set mode to {}, Integrated and Smart modes require a offload discrete GPU (not supported on desktops where the discrete GPU is the primary display)",
                        mode
                    );
                    error!("{}", error_message);
                    return Err(fdo::Error::NotSupported(error_message));
                }

                // Never block a GPU that is currently driving a connected display, the display
                // would go black.
                let offload_card = match available
                    .iter()
                    .find(|(_, discrete, default, _)| *discrete && !*default)
                {
                    Some((_, _, _, card)) => *card,
                    // has_offload_dgpu already guaranteed a matching GPU, fail gracefully anyway
                    None => {
                        return Err(fdo::Error::Failed(
                            "offload GPU disappeared during mode switch".to_string(),
                        ));
                    }
                };
                let connected = tokio::task::spawn_blocking(move || {
                    crate::core::gpu::external_display_connected(offload_card)
                })
                .await
                .map_err(|err| fdo::Error::Failed(format!("DRM probe task failed: {err}")))?
                .map_err(|err| {
                    fdo::Error::Failed(format!("failed to read DRM connector state: {err}"))
                })?;
                if connected {
                    let error_message = format!(
                        "Couldn't set mode to {}, the offload GPU is driving a connected display",
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
                    } else {
                        // unblock default GPU
                        gpu.unblock_gpu().await?;
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
        // try to restart nvidia-powerd, if error just ignore it
        task::spawn(ModeInterface::restart_nvidia_powerd());

        info!("Switched to {}", mode);
        Ok(())
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Mode")]
impl ModeInterface {
    /// Set the user-requested GPU mode over D-Bus and persist it to state file.
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        self.internal_set_mode(mode, None).await?;
        Ok(())
    }

    /// Return the GPU mode currently applied to hardware and eBPF maps.
    #[zbus(property)]
    pub(crate) async fn mode(&self) -> fdo::Result<u32> {
        Ok(u32::from(self.mode_state.read().await.mode()))
    }
}
