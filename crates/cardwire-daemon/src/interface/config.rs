use std::sync::Arc;

use crate::file::CardwireConfig;
use tokio::sync::RwLock;
use zbus::{fdo, interface};

pub struct ConfigMemory {
    pub auto_apply_gpu_state: Arc<RwLock<bool>>,
    pub experimental_nvidia_block: Arc<RwLock<bool>>,
    pub battery_auto_switch: Arc<RwLock<bool>>,
}
impl ConfigMemory {
    pub fn build(user_config: CardwireConfig) -> ConfigMemory {
        let auto_apply_gpu_state = Arc::new(RwLock::new(user_config.auto_apply_gpu_state()));
        let experimental_nvidia_block =
            Arc::new(RwLock::new(user_config.experimental_nvidia_block()));
        let battery_auto_switch = Arc::new(RwLock::new(user_config.battery_auto_switch()));

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
        let current_config = self.config.auto_apply_gpu_state.read().await;
        Ok(*current_config)
    }
    #[zbus(property)]
    pub async fn set_auto_apply_gpu_state(&mut self, state: bool) -> fdo::Result<()> {
        let mut current_config = self.config.auto_apply_gpu_state.write().await;
        *current_config = state;
        Ok(())
    }
    #[zbus(property)]
    pub async fn experimental_nvidia_block(&self) -> fdo::Result<bool> {
        let current_config = self.config.experimental_nvidia_block.read().await;
        Ok(*current_config)
    }
    #[zbus(property)]
    pub async fn set_experimental_nvidia_block(&mut self, state: bool) -> fdo::Result<()> {
        let mut current_config = self.config.experimental_nvidia_block.write().await;
        *current_config = state;
        Ok(())
    }
    #[zbus(property)]
    pub async fn battery_auto_switch(&self) -> fdo::Result<bool> {
        let current_config = self.config.battery_auto_switch.read().await;
        Ok(*current_config)
    }
    #[zbus(property)]
    pub async fn set_battery_auto_switch(&mut self, state: bool) -> fdo::Result<()> {
        let mut current_config = self.config.battery_auto_switch.write().await;
        *current_config = state;
        Ok(())
    }
}
