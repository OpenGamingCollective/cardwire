use crate::models::Modes;
use log::warn;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
const CONFIG_PATH: &str = "/var/lib/cardwire/cardwire.toml";

#[derive(Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub mode: Modes,
    #[serde(default)]
    pub block_vulkan: bool,
}

impl Config {
    pub async fn new() -> Config {
        let config_path = Path::new(self::CONFIG_PATH);
        match fs::read_to_string(config_path).await {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(err) => {
                    warn!(
                        "Invalid config file at {}: {}. Recreating default config.",
                        CONFIG_PATH, err
                    );
                    let config = Config::default();
                    if let Err(save_err) = config.save_config() {
                        warn!(
                            "Failed to save default config to {}: {}",
                            CONFIG_PATH, save_err
                        );
                    }
                    config
                }
            },
            Err(err) => {
                warn!(
                    "Could not read config at {}: {}. Creating default config.",
                    CONFIG_PATH, err
                );
                let config = Config::default();
                if let Err(save_err) = config.save_config() {
                    warn!(
                        "Failed to save default config to {}: {}",
                        CONFIG_PATH, save_err
                    );
                }
                config
            }
        }
    }

    pub fn save_config(&self) -> anyhow::Result<()> {
        let toml: String = toml::to_string(self)?;
        let config_path = PathBuf::from(CONFIG_PATH);
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(CONFIG_PATH, toml)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Default to manual mode,
            // it is the most safe option since it doesnt assume the laptop/workstation
            // configuration
            mode: Modes::Manual,
            block_vulkan: false,
        }
    }
}
