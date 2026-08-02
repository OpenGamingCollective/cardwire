//! Define the mode dbus
use crate::{
    core::gpu::external_display_connected, file::{CardwireGpuState, CardwireModeState}, interface::{GpuInterface, config::ConfigMemory}
};
use anyhow::Result;
use aya::maps::Array as AyaArray;
use cardwire_ebpf_userspace::EbpfBlocker;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap, fmt, process::Stdio, sync::{Arc, atomic::Ordering}
};
use tokio::{
    process::Command, sync::{Mutex, RwLock}, task
};
use zbus::{fdo, interface, object_server::InterfaceRef};

#[derive(Deserialize, Serialize, PartialEq, zbus::zvariant::Type, Clone, Copy, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Modes {
    Integrated,
    Hybrid,
    #[default]
    Manual,
    Smart,
}

impl fmt::Display for Modes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Modes::Integrated => write!(f, "Integrated"),
            Modes::Hybrid => write!(f, "Hybrid"),
            Modes::Manual => write!(f, "Manual"),
            Modes::Smart => write!(f, "Smart"),
        }
    }
}

/// try to convert a u32 into a mode
impl TryFrom<u32> for Modes {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Integrated),
            1 => Ok(Self::Hybrid),
            2 => Ok(Self::Manual),
            3 => Ok(Self::Smart),
            _ => Err("unknown mode"),
        }
    }
}

/// Convert a mode into a u32 and reverse
impl From<Modes> for u32 {
    fn from(value: Modes) -> Self {
        match value {
            Modes::Integrated => 0,
            Modes::Hybrid => 1,
            Modes::Manual => 2,
            Modes::Smart => 3,
        }
    }
}

// to change a mode, we need the config, the mode_state, the gpu_list
#[derive(Clone)]
pub struct ModeInterface {
    mode_state: Arc<RwLock<CardwireModeState>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
    gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    config: Arc<ConfigMemory>,
    mode_map: Arc<Mutex<AyaArray<aya::maps::MapData, u8>>>,
    transition_lock: Arc<Mutex<()>>,
    // `mode_state` is the persisted mode the user wants restored. This tracks the
    // mode currently applied to the hardware while an external display overrides it.
    effective_mode: Arc<Mutex<Modes>>,
}

impl ModeInterface {
    fn external_display_target(requested: Modes, connected: bool, enabled: bool) -> Modes {
        if enabled && connected && matches!(requested, Modes::Integrated | Modes::Smart) {
            Modes::Hybrid
        } else {
            requested
        }
    }

    pub async fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<EbpfBlocker>>,
    ) -> Result<ModeInterface> {
        let effective_mode = mode_state.read().await.mode();
        let mut blocker = blocker.write().await;
        let mode_map: aya::maps::Array<aya::maps::MapData, u8> = blocker.get_mode_map()?;
        let mode_map = Arc::new(Mutex::new(mode_map));
        Ok(ModeInterface {
            mode_state,
            gpu_state,
            gpu_list,
            config,
            mode_map,
            transition_lock: Arc::new(Mutex::new(())),
            effective_mode: Arc::new(Mutex::new(effective_mode)),
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

    async fn required_external_card(&self) -> fdo::Result<Option<u32>> {
        let card = {
            let gpu_list = self.gpu_list.read().await;
            if gpu_list.len() != 2 {
                return Ok(None);
            }

            let mut default_count = 0;
            let mut non_default_card = None;
            for gpu in gpu_list.values() {
                if gpu.device.is_default() {
                    default_count += 1;
                } else if non_default_card.replace(*gpu.device.card()).is_some() {
                    return Ok(None);
                }
            }
            if default_count != 1 {
                return Ok(None);
            }
            non_default_card
        };

        match card {
            Some(card)
                if external_display_connected(card).map_err(|err| {
                    fdo::Error::Failed(format!("failed to read DRM connector state: {err}"))
                })? =>
            {
                Ok(Some(card))
            }
            _ => Ok(None),
        }
    }

    pub async fn required_external_display_connected(&self) -> fdo::Result<bool> {
        self.required_external_card()
            .await
            .map(|card| card.is_some())
    }

    async fn notify_drm_change(card: u32) {
        let path = format!("/sys/class/drm/card{card}/uevent");
        if let Err(err) = tokio::fs::write(&path, "change\n").await {
            warn!("failed to replay DRM change event through {path}: {err}");
        }
    }

    pub async fn current_mode_value(&self) -> Modes {
        *self.effective_mode.lock().await
    }

    async fn desired_mode_value(&self) -> Modes {
        self.mode_state.read().await.mode()
    }

    async fn apply_effective_mode_locked(
        &self,
        mode: Modes,
        force_apply: bool,
    ) -> fdo::Result<bool> {
        let previous_mode = self.current_mode_value().await;
        if force_apply || mode != previous_mode {
            if let Err(err) = self.apply_mode(mode).await {
                if let Err(rollback_err) = self.apply_mode(previous_mode).await {
                    warn!(
                        "failed to restore previous mode ({previous_mode}) after apply_mode error: {rollback_err}"
                    );
                }
                return Err(err);
            }
            *self.effective_mode.lock().await = mode;
        }
        Ok(mode != previous_mode)
    }

    async fn save_desired_mode_locked(&self, mode: Modes) {
        let mut state = self.mode_state.write().await;
        if state.mode() != mode
            && let Err(err) = state.save_state(mode).await
        {
            warn!("mode couldn't be saved to state: {err}");
        }
    }

    pub fn external_display_auto_switch_enabled(&self) -> bool {
        self.config
            .external_display_auto_switch
            .load(Ordering::Relaxed)
    }

    pub async fn external_display_monitor_enabled(&self) -> bool {
        self.external_display_auto_switch_enabled()
            && self.desired_mode_value().await != Modes::Manual
    }

    async fn apply_requested_mode_locked(
        &self,
        requested: Modes,
        force_apply: bool,
    ) -> fdo::Result<bool> {
        let enabled = self.external_display_auto_switch_enabled();
        let card = if enabled && matches!(requested, Modes::Integrated | Modes::Smart) {
            self.required_external_card().await?
        } else {
            None
        };
        let target = Self::external_display_target(requested, card.is_some(), enabled);
        let changed = self
            .apply_effective_mode_locked(target, force_apply)
            .await?;
        if changed
            && target == Modes::Hybrid
            && requested != Modes::Hybrid
            && let Some(card) = card
        {
            Self::notify_drm_change(card).await;
        }
        Ok(changed)
    }

    async fn set_requested_mode_value_locked(&self, mode: Modes) -> fdo::Result<bool> {
        let changed = self.apply_requested_mode_locked(mode, false).await?;
        self.save_desired_mode_locked(mode).await;
        Ok(changed)
    }

    pub async fn reconcile_external_display(&self) -> fdo::Result<bool> {
        let _transition = self.transition_lock.lock().await;
        let requested = self.desired_mode_value().await;
        self.apply_requested_mode_locked(requested, false).await
    }

    pub async fn emit_mode_change(
        &self,
        interface: &InterfaceRef<ModeInterface>,
        changed: bool,
    ) -> zbus::Result<()> {
        if changed {
            self.mode_changed(interface.signal_emitter()).await?;
        }
        Ok(())
    }

    pub async fn apply_at_startup(&self) -> fdo::Result<()> {
        let requested = self.desired_mode_value().await;
        let enabled = self.external_display_auto_switch_enabled();
        let card = if enabled && matches!(requested, Modes::Integrated | Modes::Smart) {
            match self.required_external_card().await {
                Ok(card) => card,
                Err(err) => {
                    warn!("failed to read external display topology at startup: {err}");
                    None
                }
            }
        } else {
            None
        };
        let target = Self::external_display_target(requested, card.is_some(), enabled);
        let changed = self.apply_effective_mode_locked(target, true).await?;
        if changed
            && target == Modes::Hybrid
            && requested != Modes::Hybrid
            && let Some(card) = card
        {
            Self::notify_drm_change(card).await;
        }
        Ok(())
    }

    pub async fn apply_startup_fallback(&self) -> fdo::Result<()> {
        let _transition = self.transition_lock.lock().await;
        self.apply_effective_mode_locked(Modes::Manual, true)
            .await?;
        self.save_desired_mode_locked(Modes::Manual).await;
        Ok(())
    }

    pub(crate) async fn apply_mode(&self, mode: Modes) -> fdo::Result<()> {
        let mut gpu_list = self.gpu_list.write().await;
        match mode {
            // Integrated/Hybrid/Smart only works on laptop with two gpus, will refuse if the
            // computer has more than 2 gpus
            Modes::Integrated | Modes::Hybrid | Modes::Smart => {
                if gpu_list.len() != 2 {
                    let error_message = format!(
                        "Couldn't set mode to {}, the mode require exactly 2 GPUs",
                        mode
                    );
                    error!("{}", error_message);
                    return Err(fdo::Error::NotSupported(error_message.to_string()));
                }
                // Loop to find the non default gpu and block it,
                for (id, gpu) in gpu_list.iter_mut() {
                    if !gpu.device.is_default() {
                        if mode == Modes::Integrated || mode == Modes::Smart {
                            // Here we block the dGPU
                            gpu.block_gpu(*id as u32).await?;
                        } else {
                            gpu.unblock_gpu().await?;
                        }
                    } else if mode == Modes::Smart && gpu.device.is_default() {
                        // push default gpu (iGPU) into the blocked inode map for tracking only
                        gpu.block_gpu(*id as u32).await?;
                    } else {
                        // clear
                        gpu.unblock_gpu().await?;
                    }
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
                for (id, gpu) in gpu_list.iter_mut() {
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
    /*
        Set the mode
    */
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        let _transition = self.transition_lock.lock().await;
        self.set_requested_mode_value_locked(mode).await?;
        Ok(())
    }
    #[zbus(property)]
    pub(crate) async fn mode(&self) -> fdo::Result<u32> {
        Ok(self.current_mode_value().await.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modes_try_from_valid_values() {
        assert_eq!(Modes::try_from(0).unwrap(), Modes::Integrated);
        assert_eq!(Modes::try_from(1).unwrap(), Modes::Hybrid);
        assert_eq!(Modes::try_from(2).unwrap(), Modes::Manual);
        assert_eq!(Modes::try_from(3).unwrap(), Modes::Smart);
    }

    #[test]
    fn test_modes_try_from_invalid_value() {
        assert!(Modes::try_from(4).is_err());
        assert!(Modes::try_from(u32::MAX).is_err());
    }

    #[test]
    fn test_modes_into_u32_roundtrip() {
        for i in 0..=3u32 {
            let mode = Modes::try_from(i).unwrap();
            let back: u32 = mode.into();
            assert_eq!(back, i);
        }
    }

    #[test]
    fn test_modes_display_formatting() {
        assert_eq!(Modes::Integrated.to_string(), "Integrated");
        assert_eq!(Modes::Hybrid.to_string(), "Hybrid");
        assert_eq!(Modes::Manual.to_string(), "Manual");
        assert_eq!(Modes::Smart.to_string(), "Smart");
    }

    #[test]
    fn test_modes_default_is_manual() {
        assert_eq!(Modes::default(), Modes::Manual);
    }

    #[test]
    fn test_modes_serde_json_roundtrip() {
        let modes = [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ];
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: Modes = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, mode);
        }
    }

    #[test]
    fn test_modes_serde_uses_snake_case() {
        let json = serde_json::to_string(&Modes::Integrated).unwrap();
        assert_eq!(json, "\"integrated\"");
        let json = serde_json::to_string(&Modes::Hybrid).unwrap();
        assert_eq!(json, "\"hybrid\"");
    }

    #[test]
    fn external_display_only_overrides_integrated_and_smart() {
        for mode in [Modes::Integrated, Modes::Smart] {
            assert_eq!(
                ModeInterface::external_display_target(mode, true, true),
                Modes::Hybrid
            );
        }
        for mode in [Modes::Hybrid, Modes::Manual] {
            assert_eq!(
                ModeInterface::external_display_target(mode, true, true),
                mode
            );
        }
        for mode in [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ] {
            assert_eq!(
                ModeInterface::external_display_target(mode, true, false),
                mode
            );
            assert_eq!(
                ModeInterface::external_display_target(mode, false, true),
                mode
            );
        }
    }
}
