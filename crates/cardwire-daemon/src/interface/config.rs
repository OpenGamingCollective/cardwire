use std::sync::{
    Arc, atomic::{AtomicBool, Ordering}
};

use crate::file::CardwireConfig;
use zbus::{fdo, interface};

// Use a custom Config struct instead of CarwireConfig to allow more control over the settings
pub struct ConfigMemory {
    pub auto_apply_gpu_state: Arc<AtomicBool>,
    pub experimental_nvidia_block: Arc<AtomicBool>,
    pub battery_auto_switch: Arc<AtomicBool>,
}
impl ConfigMemory {
    /// build a ConfigMemory from CardwireConfig
    pub fn build(user_config: CardwireConfig) -> ConfigMemory {
        let auto_apply_gpu_state = Arc::new(AtomicBool::new(user_config.auto_apply_gpu_state()));
        let experimental_nvidia_block =
            Arc::new(AtomicBool::new(user_config.experimental_nvidia_block()));
        let battery_auto_switch = Arc::new(AtomicBool::new(user_config.battery_auto_switch()));

        ConfigMemory {
            auto_apply_gpu_state,
            experimental_nvidia_block,
            battery_auto_switch,
        }
    }
}

#[derive(Clone)]
pub struct ConfigInterface {
    pub config: Arc<ConfigMemory>,
}
impl ConfigInterface {
    pub fn build(config: Arc<ConfigMemory>) -> anyhow::Result<ConfigInterface> {
        Ok(Self { config })
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Config")]
impl ConfigInterface {
    #[zbus(property)]
    pub async fn auto_apply_gpu_state(&self) -> fdo::Result<bool> {
        Ok(self.config.auto_apply_gpu_state.load(Ordering::Relaxed))
    }
    #[zbus(property)]
    pub async fn set_auto_apply_gpu_state(&mut self, state: bool) -> fdo::Result<()> {
        self.config
            .auto_apply_gpu_state
            .store(state, Ordering::Relaxed);
        Ok(())
    }
    #[zbus(property)]
    pub async fn experimental_nvidia_block(&self) -> fdo::Result<bool> {
        Ok(self
            .config
            .experimental_nvidia_block
            .load(Ordering::Relaxed))
    }
    #[zbus(property)]
    pub async fn set_experimental_nvidia_block(&mut self, state: bool) -> fdo::Result<()> {
        self.config
            .experimental_nvidia_block
            .store(state, Ordering::Relaxed);
        Ok(())
    }
    #[zbus(property)]
    pub async fn battery_auto_switch(&self) -> fdo::Result<bool> {
        Ok(self.config.battery_auto_switch.load(Ordering::Relaxed))
    }
    #[zbus(property)]
    pub async fn set_battery_auto_switch(&mut self, state: bool) -> fdo::Result<()> {
        self.config
            .battery_auto_switch
            .store(state, Ordering::Relaxed);
        Ok(())
    }
    /// Save the daemon's configuration to cardwire.toml
    pub async fn save_to_file(&self) -> fdo::Result<()> {
        let config = CardwireConfig::new(
            self.config.auto_apply_gpu_state.load(Ordering::Relaxed),
            self.config
                .experimental_nvidia_block
                .load(Ordering::Relaxed),
            self.config.battery_auto_switch.load(Ordering::Relaxed),
        );
        config.save_config().await?;
        Ok(())
    }
}
