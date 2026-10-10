use std::{io::ErrorKind, sync::Arc};

use crate::{Result, file::CardwireConfig, interface::DaemonContext};
use cardwire_core::modes::types::Modes;
use cardwire_ebpf_userspace::{EbpfBlocker, EbpfSettings};
use log::warn;
use tokio::sync::RwLock;
use zbus::{fdo, interface};

#[derive(Clone)]
pub struct ConfigInterface {
    config: Arc<RwLock<CardwireConfig>>,
    blocker: Arc<RwLock<EbpfBlocker>>,
}
impl ConfigInterface {
    pub fn build(context: &DaemonContext) -> Result<ConfigInterface> {
        Ok(Self {
            config: context.config.clone(),
            blocker: context.blocker.clone(),
        })
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Config")]
impl ConfigInterface {
    // getters
    #[zbus(property)]
    pub async fn auto_apply_gpu_state(&self) -> fdo::Result<bool> {
        let config = self.config.read().await;
        Ok(config.global_settings.restore_gpu_states)
    }
    #[zbus(property)]
    pub async fn experimental_nvidia_block(&self) -> fdo::Result<bool> {
        let config = self.config.read().await;
        Ok(config.experimental_features.advanced_nvidia_blocking)
    }

    #[zbus(property)]
    pub async fn battery_auto_switch(&self) -> fdo::Result<bool> {
        let config = self.config.read().await;
        Ok(config.global_settings.battery_switch.enabled)
    }

    #[zbus(property)]
    pub async fn battery_auto_switch_mode(&self) -> fdo::Result<u32> {
        let config = self.config.read().await;
        Ok(config.global_settings.battery_switch.ac_mode.into())
    }

    #[zbus(property)]
    pub async fn external_display_auto_switch(&self) -> fdo::Result<bool> {
        let config = self.config.read().await;
        Ok(config.global_settings.switch_on_display)
    }

    // setters
    #[zbus(property)]
    pub async fn set_auto_apply_gpu_state(&mut self, state: bool) -> fdo::Result<()> {
        let mut config = self.config.write().await;
        config.global_settings.restore_gpu_states = state;
        self.save_to_file(config).await?;
        Ok(())
    }

    #[zbus(property)]
    pub async fn set_experimental_nvidia_block(&mut self, state: bool) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        // change the value in the ebpf map
        blocker
            .set_ebpf_setting(EbpfSettings::ExperimentalNvidia, state.into())
            .map_err(|e| fdo::Error::Failed(format!("failed to set nvidia block: {}", e)))?;
        // Save the config if the bpf map didnt return any error
        let mut config = self.config.write().await;
        config.experimental_features.advanced_nvidia_blocking = state;
        self.save_to_file(config).await?;
        Ok(())
    }

    #[zbus(property)]
    pub async fn set_battery_auto_switch(&mut self, state: bool) -> fdo::Result<()> {
        let mut config = self.config.write().await;
        config.global_settings.battery_switch.enabled = state;
        self.save_to_file(config).await?;
        Ok(())
    }
    #[zbus(property)]
    pub async fn set_battery_auto_switch_mode(&self, mode: u32) -> fdo::Result<()> {
        // Validate before storing so an invalid value can't poison the daemon
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        let mut config = self.config.write().await;
        config.global_settings.battery_switch.ac_mode = mode;
        self.save_to_file(config).await?;
        Ok(())
    }
    #[zbus(property)]
    pub async fn set_external_display_auto_switch(&mut self, _state: bool) -> fdo::Result<()> {
        // No-Op since this feature is not stable yet
        Ok(())
    }
}
impl ConfigInterface {
    /// Save the daemon's configuration to cardwire.toml
    // lock is passed to this function to prevent deadlocks
    pub async fn save_to_file(
        &self,
        config: tokio::sync::RwLockWriteGuard<'_, CardwireConfig>,
    ) -> fdo::Result<()> {
        // Save to file, if the file is read-only, only warn and return OK, this happen on nixos
        // because of the immutable nature of the distribution (maybe other distros can be included)
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
