//! Define the mode dbus
use crate::{
    core::gpu::connected_external_drm_cards, file::{CardwireGpuState, CardwireModeState}, interface::{GpuInterface, config::ConfigMemory}
};
use anyhow::Result;
use aya::maps::HashMap as AyaHashMap;
use cardwire_ebpf::EbpfBlocker;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet}, fmt, process::Stdio, sync::{Arc, atomic::Ordering}
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
    mode_map: Arc<Mutex<AyaHashMap<aya::maps::MapData, u8, u8>>>,
    transition_lock: Arc<Mutex<()>>,
}

impl ModeInterface {
    fn select_external_display_mode(connected: bool, restore_mode: Modes) -> Modes {
        if connected {
            Modes::Hybrid
        } else {
            restore_mode
        }
    }

    pub async fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<EbpfBlocker>>,
    ) -> Result<ModeInterface> {
        let mut blocker = blocker.write().await;
        let mode_map: aya::maps::HashMap<aya::maps::MapData, u8, u8> = blocker.get_mode_map()?;
        let mode_map = Arc::new(Mutex::new(mode_map));
        Ok(ModeInterface {
            mode_state,
            gpu_state,
            gpu_list,
            config,
            mode_map,
            transition_lock: Arc::new(Mutex::new(())),
        })
    }

    async fn required_external_cards(&self) -> fdo::Result<BTreeSet<u32>> {
        let connected_cards = connected_external_drm_cards().map_err(|err| {
            fdo::Error::Failed(format!("failed to read DRM connector state: {err}"))
        })?;
        let gpu_list = self.gpu_list.read().await;
        Ok(gpu_list
            .values()
            .filter(|gpu| !gpu.device.is_default() && connected_cards.contains(gpu.device.card()))
            .map(|gpu| *gpu.device.card())
            .collect())
    }

    async fn notify_drm_change(cards: &BTreeSet<u32>) {
        for card in cards {
            let path = format!("/sys/class/drm/card{card}/uevent");
            if let Err(err) = tokio::fs::write(&path, "change\n").await {
                warn!("failed to replay DRM change event through {path}: {err}");
            }
        }
    }

    pub async fn current_mode_value(&self) -> Modes {
        self.mode_state.read().await.mode()
    }

    pub async fn set_mode_value(&self, mode: Modes, force_apply: bool) -> fdo::Result<bool> {
        let _transition = self.transition_lock.lock().await;
        self.set_mode_value_locked(mode, force_apply).await
    }

    async fn set_mode_value_locked(&self, mode: Modes, force_apply: bool) -> fdo::Result<bool> {
        let previous_mode = self.current_mode_value().await;
        if force_apply || mode != previous_mode {
            self.apply_mode(mode).await?;
            let mut state = self.mode_state.write().await;
            if let Err(err) = state.save_state(mode).await {
                warn!("mode couldn't be saved to state: {err}");
            }
        }
        Ok(mode != previous_mode)
    }

    pub fn external_display_auto_switch_enabled(&self) -> bool {
        self.config
            .external_display_auto_switch
            .load(Ordering::Relaxed)
    }

    fn external_display_restore_mode(&self) -> fdo::Result<Modes> {
        Modes::try_from(
            self.config
                .external_display_auto_switch_mode
                .load(Ordering::Relaxed),
        )
        .map_err(|err| fdo::Error::Failed(err.to_string()))
    }

    pub async fn required_external_display_connected(&self) -> fdo::Result<bool> {
        self.required_external_cards()
            .await
            .map(|cards| !cards.is_empty())
    }

    async fn external_display_target_mode(&self, connected: bool) -> fdo::Result<Modes> {
        let restore_mode = self.external_display_restore_mode()?;
        Ok(Self::select_external_display_mode(connected, restore_mode))
    }

    pub async fn apply_external_display_mode(&self, connected: bool) -> fdo::Result<bool> {
        let _transition = self.transition_lock.lock().await;
        if !self.external_display_auto_switch_enabled() {
            return Ok(false);
        }
        let cards = if connected {
            self.required_external_cards().await?
        } else {
            BTreeSet::new()
        };
        let mode = self.external_display_target_mode(connected).await?;
        let changed = self.set_mode_value_locked(mode, false).await?;
        if connected && changed {
            Self::notify_drm_change(&cards).await;
        }
        Ok(changed)
    }

    pub async fn external_display_setting_changed(
        &self,
        enabled: bool,
    ) -> fdo::Result<(bool, Modes)> {
        let _transition = self.transition_lock.lock().await;
        let previous_mode = self.current_mode_value().await;
        let was_enabled = self.external_display_auto_switch_enabled();
        self.config
            .external_display_auto_switch
            .store(enabled, Ordering::Relaxed);
        if !enabled || was_enabled {
            return Ok((false, previous_mode));
        }

        let res = async {
            let cards = self.required_external_cards().await?;
            let connected = !cards.is_empty();
            let mode = self.external_display_target_mode(connected).await?;
            let changed = self.set_mode_value_locked(mode, false).await?;
            if connected && changed {
                Self::notify_drm_change(&cards).await;
            }
            Ok((changed, previous_mode))
        }
        .await;

        if res.is_err() {
            self.config
                .external_display_auto_switch
                .store(was_enabled, Ordering::Relaxed);
        }

        res
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
        let (mode, connected_cards) = if self.external_display_auto_switch_enabled() {
            match self.required_external_cards().await {
                Ok(cards) => {
                    let mode = self.external_display_target_mode(!cards.is_empty()).await?;
                    (mode, cards)
                }
                Err(err) => {
                    warn!("failed to read external card topology at startup: {err}");
                    (self.current_mode_value().await, BTreeSet::new())
                }
            }
        } else {
            (self.current_mode_value().await, BTreeSet::new())
        };
        self.set_mode_value(mode, true).await?;
        if !connected_cards.is_empty() {
            Self::notify_drm_change(&connected_cards).await;
        }
        Ok(())
    }

    pub async fn apply_startup_fallback(&self) -> fdo::Result<()> {
        self.set_mode_value(Modes::Manual, true).await.map(|_| ())
    }

    /// set the mode in the `cardwire_mode` bpf map
    async fn update_mode_bpf_map(&self, mode: Modes) -> fdo::Result<()> {
        let mut mode_map = self.mode_map.lock().await;
        let mode: u32 = Modes::into(mode);
        mode_map
            .insert(0, mode as u8, 0)
            .map_err(|err| fdo::Error::Failed(err.to_string()))
    }

    async fn apply_mode(&self, mode: Modes) -> fdo::Result<()> {
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
                for gpu in gpu_list.values_mut() {
                    if !gpu.device.is_default() {
                        if mode == Modes::Integrated || mode == Modes::Smart {
                            // Here we block the dGPU
                            gpu.block_gpu(1).await?;
                        } else {
                            gpu.unblock_gpu().await?;
                        }
                    } else if mode == Modes::Smart {
                        // push default gpu (iGPU) into the blocked inode map for tracking only
                        gpu.block_gpu(0).await?;
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
                for (_, gpu) in gpu_list.iter_mut() {
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
                            gpu.block_gpu(1).await?;
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
}

#[interface(name = "com.github.opengamingcollective.cardwire.Mode")]
impl ModeInterface {
    /*
        Set the mode
    */
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        self.set_mode_value(mode, false).await?;
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
    fn external_display_target_uses_hybrid_until_disconnect() {
        for restore_mode in [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ] {
            assert_eq!(
                ModeInterface::select_external_display_mode(true, restore_mode),
                Modes::Hybrid
            );
            assert_eq!(
                ModeInterface::select_external_display_mode(false, restore_mode),
                restore_mode
            );
        }
    }
}
