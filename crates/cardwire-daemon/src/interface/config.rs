use std::{
    io::ErrorKind, sync::{
        Arc, atomic::{AtomicBool, AtomicU32, Ordering}
    }
};

use crate::{file::CardwireConfig, interface::Modes};
use cardwire_ebpf_userspace::{EbpfBlocker, EbpfSettings};
use log::warn;
use tokio::sync::RwLock;
use zbus::{fdo, interface};

// Use a custom Config struct instead of CarwireConfig to allow more control over the settings
pub struct ConfigMemory {
    pub auto_apply_gpu_state: Arc<AtomicBool>,
    pub experimental_nvidia_block: Arc<AtomicBool>,
    pub battery_auto_switch: Arc<AtomicBool>,
    pub battery_auto_switch_mode: Arc<AtomicU32>,
    pub external_display_auto_switch: Arc<AtomicBool>,
}
impl ConfigMemory {
    /// build a ConfigMemory from CardwireConfig
    pub fn build(user_config: CardwireConfig) -> ConfigMemory {
        let auto_apply_gpu_state = Arc::new(AtomicBool::new(user_config.auto_apply_gpu_state()));
        let experimental_nvidia_block =
            Arc::new(AtomicBool::new(user_config.experimental_nvidia_block()));
        let battery_auto_switch = Arc::new(AtomicBool::new(user_config.battery_auto_switch()));
        let battery_auto_switch_mode = Arc::new(AtomicU32::new(
            user_config.battery_auto_switch_mode().into(),
        ));
        let external_display_auto_switch =
            Arc::new(AtomicBool::new(user_config.external_display_auto_switch()));
        ConfigMemory {
            auto_apply_gpu_state,
            experimental_nvidia_block,
            battery_auto_switch,
            battery_auto_switch_mode,
            external_display_auto_switch,
        }
    }
}

#[derive(Clone)]
pub struct ConfigInterface {
    config: Arc<ConfigMemory>,
    blocker: Arc<RwLock<EbpfBlocker>>,
}
impl ConfigInterface {
    pub fn build(
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<EbpfBlocker>>,
    ) -> anyhow::Result<ConfigInterface> {
        Ok(Self { config, blocker })
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Config")]
impl ConfigInterface {
    // getters
    #[zbus(property)]
    pub async fn auto_apply_gpu_state(&self) -> fdo::Result<bool> {
        Ok(self.config.auto_apply_gpu_state.load(Ordering::Relaxed))
    }
    #[zbus(property)]
    pub async fn experimental_nvidia_block(&self) -> fdo::Result<bool> {
        Ok(self
            .config
            .experimental_nvidia_block
            .load(Ordering::Relaxed))
    }
    #[zbus(property)]
    pub async fn battery_auto_switch(&self) -> fdo::Result<bool> {
        Ok(self.config.battery_auto_switch.load(Ordering::Relaxed))
    }
    #[zbus(property)]
    pub async fn battery_auto_switch_mode(&self) -> fdo::Result<u32> {
        let mode = self.config.battery_auto_switch_mode.load(Ordering::Relaxed);
        Ok(mode)
    }

    // setters
    #[zbus(property)]
    pub async fn set_auto_apply_gpu_state(&mut self, state: bool) -> fdo::Result<()> {
        self.config
            .auto_apply_gpu_state
            .store(state, Ordering::Relaxed);
        self.save_to_file().await?;
        Ok(())
    }
    #[zbus(property)]
    pub async fn set_experimental_nvidia_block(&mut self, state: bool) -> fdo::Result<()> {
        self.config
            .experimental_nvidia_block
            .store(state, Ordering::Relaxed);
        let mut blocker = self.blocker.write().await;
        // change the value in the ebpf map
        blocker
            .set_ebpf_setting(EbpfSettings::ExperimentalNvidia, state.into())
            .map_err(|e| fdo::Error::Failed(format!("failed to set nvidia block: {}", e)))?;
        self.save_to_file().await?;
        Ok(())
    }

    #[zbus(property)]
    pub async fn set_battery_auto_switch(&mut self, state: bool) -> fdo::Result<()> {
        self.config
            .battery_auto_switch
            .store(state, Ordering::Relaxed);
        self.save_to_file().await?;
        Ok(())
    }
    #[zbus(property)]
    pub async fn set_battery_auto_switch_mode(&self, mode: u32) -> fdo::Result<()> {
        // Validate before storing so an invalid value can't poison the in-memory state
        Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        self.config
            .battery_auto_switch_mode
            .store(mode, Ordering::Relaxed);
        self.save_to_file().await?;
        Ok(())
    }
    /// Save the daemon's configuration to cardwire.toml
    pub async fn save_to_file(&self) -> fdo::Result<()> {
        // Include monitor-owned settings whenever any D-Bus property rewrites the whole file.
        let config = CardwireConfig::new(
            self.config.auto_apply_gpu_state.load(Ordering::Relaxed),
            self.config
                .experimental_nvidia_block
                .load(Ordering::Relaxed),
            self.config.battery_auto_switch.load(Ordering::Relaxed),
            Modes::try_from(self.config.battery_auto_switch_mode.load(Ordering::Relaxed))
                .map_err(|err| fdo::Error::Failed(err.to_string()))?,
            self.config
                .external_display_auto_switch
                .load(Ordering::Relaxed),
        );
        match config.save_config().await {
            Ok(_) => Ok(()),
            Err(err) => match err.kind() {
                ErrorKind::ReadOnlyFilesystem => {
                    warn!(
                        "IO Error in save_config: {}, ignoring, system might be nix or bootc",
                        err
                    );
                    Ok(())
                }
                _ => Err(fdo::Error::Failed(err.to_string())),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::CardwireConfig;

    #[test]
    fn test_config_memory_build_from_default_config() {
        let config = CardwireConfig::default();
        let memory = ConfigMemory::build(config);
        assert!(memory.auto_apply_gpu_state.load(Ordering::Relaxed));
        assert!(!memory.experimental_nvidia_block.load(Ordering::Relaxed));
        assert!(!memory.battery_auto_switch.load(Ordering::Relaxed));
        assert!(!memory.external_display_auto_switch.load(Ordering::Relaxed));
    }

    #[test]
    fn test_config_memory_build_from_custom_config() {
        let config = CardwireConfig::new(false, true, true, Modes::Smart, true);
        let memory = ConfigMemory::build(config);
        assert!(!memory.auto_apply_gpu_state.load(Ordering::Relaxed));
        assert!(memory.experimental_nvidia_block.load(Ordering::Relaxed));
        assert!(memory.battery_auto_switch.load(Ordering::Relaxed));
        let mode_val = memory.battery_auto_switch_mode.load(Ordering::Relaxed);
        assert_eq!(Modes::try_from(mode_val).unwrap(), Modes::Smart);
        assert!(memory.external_display_auto_switch.load(Ordering::Relaxed));
    }

    #[test]
    fn test_config_memory_atomic_store_and_load() {
        let config = CardwireConfig::default();
        let memory = ConfigMemory::build(config);
        // Mutate the atomic
        memory.auto_apply_gpu_state.store(false, Ordering::Relaxed);
        assert!(!memory.auto_apply_gpu_state.load(Ordering::Relaxed));
        memory
            .external_display_auto_switch
            .store(true, Ordering::Relaxed);
        assert!(memory.external_display_auto_switch.load(Ordering::Relaxed));
    }
}
