//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::{
    file::common::{FileKind, create_default_file}, interface::Modes
};
use anyhow::Context;

use serde::{Deserialize, Serialize};
use std::fs;
use zbus::fdo;
const CONFIG_PATH: &str = "/etc/cardwire";

#[derive(Deserialize, Serialize, Debug)]
#[serde(default)]
pub struct CardwireConfig {
    auto_apply_gpu_state: bool,
    experimental_nvidia_block: bool,
    battery_auto_switch: bool,
    battery_auto_switch_mode: Modes,
}
impl Default for CardwireConfig {
    fn default() -> Self {
        CardwireConfig {
            auto_apply_gpu_state: true,
            experimental_nvidia_block: false,
            battery_auto_switch: false,
            battery_auto_switch_mode: Modes::Hybrid,
        }
    }
}
impl CardwireConfig {
    /// used to create a new config from given values
    pub fn new(
        auto_apply_gpu_state: bool,
        experimental_nvidia_block: bool,
        battery_auto_switch: bool,
        battery_auto_switch_mode: Modes,
    ) -> CardwireConfig {
        CardwireConfig {
            auto_apply_gpu_state,
            experimental_nvidia_block,
            battery_auto_switch,
            battery_auto_switch_mode,
        }
    }
    /// Read TOML config file and return it's settings as a struct
    pub fn build() -> anyhow::Result<CardwireConfig> {
        let config_file = format!("{}/cardwire.toml", CONFIG_PATH);
        Self::parse_config(&config_file)
    }
    /// Parse the .toml file into a CardwireConfig
    fn parse_config(config_file: &str) -> anyhow::Result<CardwireConfig> {
        // create the config if it doesnt exist
        if !(fs::exists(config_file)?) {
            Self::create_default_config().context("Could not create default dir for config")?;
        }
        // read the config into a string and parse it
        let config_content =
            fs::read_to_string(config_file).context("Could not read cardwire.toml")?;
        toml::from_str(&config_content).context("Failed to parse the toml config")
    }
    /// Create a default cardwire.toml if not present
    fn create_default_config() -> anyhow::Result<()> {
        create_default_file(FileKind::Config)?;
        Ok(())
    }
    /// Save the config into cardwire.toml
    pub async fn save_config(&self) -> fdo::Result<()> {
        let path = format!("{}/cardwire.toml", CONFIG_PATH);
        match toml::to_string_pretty(&self) {
            Ok(config_toml) => {
                if let Err(e) = tokio::fs::write(path, config_toml).await {
                    return Err(fdo::Error::Failed(e.to_string()));
                }
            }
            Err(e) => return Err(fdo::Error::Failed(e.to_string())),
        };
        Ok(())
    }
    pub fn experimental_nvidia_block(&self) -> bool {
        self.experimental_nvidia_block
    }
    pub fn auto_apply_gpu_state(&self) -> bool {
        self.auto_apply_gpu_state
    }
    pub fn battery_auto_switch(&self) -> bool {
        self.battery_auto_switch
    }
    pub fn battery_auto_switch_mode(&self) -> Modes {
        self.battery_auto_switch_mode
    }
}
