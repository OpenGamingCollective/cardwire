//! Define the mode dbus
use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{DaemonContext, GpuInterface, config::ConfigMemory}
};
use anyhow::Result;
use aya::maps::Array as AyaArray;
use log::{error, info, warn};
use std::{
    collections::BTreeMap, process::Stdio, sync::{Arc, OnceLock}
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
    // Effective mode currently applied to the GPUs
    effective_mode: Arc<RwLock<Modes>>,
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
        let initial_mode = context.mode_state.read().await.mode();
        Ok(ModeInterface {
            mode_state: context.mode_state.clone(),
            gpu_state: context.gpu_state.clone(),
            gpu_list: context.gpu_list.clone(),
            config: context.config.clone(),
            mode_map,
            effective_mode: Arc::new(RwLock::new(initial_mode)),
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

    /// Return the effective mode currently applied to the GPUs.
    pub async fn current_mode_value(&self) -> Modes {
        *self.effective_mode.read().await
    }

    /// Return the user-requested mode stored in state file.
    pub async fn requested_mode_value(&self) -> Modes {
        self.mode_state.read().await.mode()
    }

    /// Persist a user-requested mode without changing a temporary effective override.
    pub(crate) async fn save_mode(&self, mode: Modes) {
        let mut current_mode = self.mode_state.write().await;
        if let Err(e) = current_mode.save_state(mode).await {
            warn!("mode couldn't be saved to config: {e}");
        }
    }

    pub async fn emit_mode_change(&self, changed: bool) -> zbus::Result<()> {
        if changed && let Some(emitter) = self.signal_emitter.get() {
            self.mode_changed(emitter).await?;
        }
        Ok(())
    }

    /// Resolve the target effective mode and DRM card for a requested mode.
    pub(crate) async fn detect_display_target(
        &self,
        requested: Modes,
    ) -> fdo::Result<(Modes, Option<u32>)> {
        crate::tasks::detect_external_display_target(&self.gpu_list, &self.config, requested).await
    }

    /// Apply an effective mode without persisting it to mode_state.
    pub async fn effective_set_mode(&self, target: Modes, force: bool) -> fdo::Result<bool> {
        let _transition = self.transition.lock().await;
        let previous = *self.effective_mode.read().await;
        if force || target != previous {
            self.apply_mode(target).await?;
            *self.effective_mode.write().await = target;
        }
        Ok(target != previous)
    }

    /// Apply and persist a user-requested mode.
    pub async fn set_requested_mode(&self, requested: Modes) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        let (target, _card) = match self.detect_display_target(requested).await {
            Ok(res) => res,
            Err(err) => {
                warn!("failed to read external display topology: {err}");
                return Err(err);
            }
        };
        let previous = *self.effective_mode.read().await;
        if target != previous {
            self.apply_mode(target).await?;
            *self.effective_mode.write().await = target;
        }
        self.save_mode(requested).await;
        Ok(())
    }

    /// Apply mode at daemon startup.
    pub async fn apply_mode_at_startup(&self, requested: Modes, force: bool) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        let (target, _card) = match self.detect_display_target(requested).await {
            Ok(target) => target,
            Err(err) => {
                warn!("failed to read external display topology at startup: {err}");
                return Err(err);
            }
        };
        let previous = *self.effective_mode.read().await;
        if force || target != previous {
            self.apply_mode(target).await?;
            *self.effective_mode.write().await = target;
        }
        Ok(())
    }

    /// Rebuild the applied state after a GPU hotplug
    pub async fn reconcile_after_hotplug(&self) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        let requested = self.requested_mode_value().await;

        let (target, _card) = match self.detect_display_target(requested).await {
            Ok(target) => target,
            Err(err) => {
                warn!("failed to resolve display target on hotplug, falling back to hybrid: {err}");
                if let Err(fb) = self.apply_mode(Modes::Hybrid).await {
                    warn!("failed to fall back to hybrid mode on hotplug: {fb}");
                    return Err(fb);
                }
                *self.effective_mode.write().await = Modes::Hybrid;
                return Err(err);
            }
        };

        if let Err(err) = self.apply_mode(target).await {
            warn!("failed to re-apply mode on hotplug, falling back to hybrid: {err}");
            if let Err(fb) = self.apply_mode(Modes::Hybrid).await {
                warn!("failed to fall back to hybrid mode on hotplug: {fb}");
                return Err(fb);
            }
            *self.effective_mode.write().await = Modes::Hybrid;
            return Ok(());
        }

        *self.effective_mode.write().await = target;
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
                let config = self
                    .config
                    .auto_apply_gpu_state
                    .load(std::sync::atomic::Ordering::Relaxed);
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
    pub(crate) async fn set_mode(
        &self,
        mode: u32,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        self.set_requested_mode(mode).await?;
        self.requested_mode_changed(&emitter).await?;
        Ok(())
    }

    /// Return the effective GPU mode currently applied to hardware and eBPF maps.
    #[zbus(property)]
    pub(crate) async fn mode(&self) -> fdo::Result<u32> {
        Ok(self.current_mode_value().await.into())
    }

    /// Return the persisted user-requested GPU mode, which may differ from `mode` during an
    /// external display override.
    #[zbus(property)]
    pub(crate) async fn requested_mode(&self) -> fdo::Result<u32> {
        Ok(self.requested_mode_value().await.into())
    }
}
