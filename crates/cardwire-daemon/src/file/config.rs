//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::file::common::{FileKind, create_default_file};
use anyhow::{Context, Ok};

use serde::{Deserialize, Serialize};
use std::fs;
const CONFIG_PATH: &str = "/etc/cardwire";

#[derive(Deserialize, Serialize, Debug)]
#[serde(default)]
pub struct CardwireConfig {
    auto_apply_gpu_state: bool,
    experimental_nvidia_block: bool,
    battery_auto_switch: bool,
}
impl Default for CardwireConfig {
    fn default() -> Self {
        CardwireConfig {
            auto_apply_gpu_state: true,
            experimental_nvidia_block: false,
            battery_auto_switch: false,
        }
    }
}
impl CardwireConfig {
    /// Read TOML config file and return it's settings as a struct
    // TODO: Error handling on std::fs
    pub fn build() -> anyhow::Result<CardwireConfig> {
        let config_file = format!("{}/cardwire.toml", CONFIG_PATH);
        Self::parse_config(&config_file)
    }
    /// Parse the .toml file into a CardwireConfig
    fn parse_config(config_file: &str) -> anyhow::Result<CardwireConfig> {
        if !(fs::exists(config_file)?) {
            Self::create_default_config().context("Could not create default dir for config")?;
        }
        let config_content =
            fs::read_to_string(config_file).context("Could not read cardwire.toml")?;
        Ok(toml::from_str(&config_content).context("Failed to parse the toml config")?)
    }
    /// Create a default cardwire.toml if not present
    fn create_default_config() -> anyhow::Result<()> {
        create_default_file(FileKind::Config)?;
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
}
