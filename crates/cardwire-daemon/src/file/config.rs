//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::{
    file::common::{FileKind, create_default_file}, interface::Modes
};
use anyhow::Context;

use serde::{Deserialize, Serialize};
use std::{fs, io};
const CONFIG_PATH: &str = "/etc/cardwire";

#[derive(Deserialize, Serialize, Debug)]
#[serde(default)]
pub struct CardwireConfig {
    auto_apply_gpu_state: bool,
    experimental_nvidia_block: bool,
    battery_auto_switch: bool,
    battery_auto_switch_mode: Modes,
    external_display_auto_switch: bool,
}
impl Default for CardwireConfig {
    fn default() -> Self {
        CardwireConfig {
            auto_apply_gpu_state: true,
            experimental_nvidia_block: false,
            battery_auto_switch: false,
            battery_auto_switch_mode: Modes::Hybrid,
            external_display_auto_switch: false,
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
        external_display_auto_switch: bool,
    ) -> CardwireConfig {
        CardwireConfig {
            auto_apply_gpu_state,
            experimental_nvidia_block,
            battery_auto_switch,
            battery_auto_switch_mode,
            external_display_auto_switch,
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
    pub async fn save_config(&self) -> io::Result<()> {
        let path = format!("{}/cardwire.toml", CONFIG_PATH);
        match toml::to_string_pretty(&self) {
            Ok(config_toml) => tokio::fs::write(path, config_toml).await,
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
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
    pub fn external_display_auto_switch(&self) -> bool {
        self.external_display_auto_switch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::Modes;

    #[test]
    fn test_cardwire_config_default_values() {
        let config = CardwireConfig::default();
        assert!(config.auto_apply_gpu_state());
        assert!(!config.experimental_nvidia_block());
        assert!(!config.battery_auto_switch());
        assert_eq!(config.battery_auto_switch_mode(), Modes::Hybrid);
        assert!(!config.external_display_auto_switch());
    }

    #[test]
    fn test_cardwire_config_build_values() {
        let config = CardwireConfig::new(false, true, true, Modes::Smart, true);
        assert!(!config.auto_apply_gpu_state());
        assert!(config.experimental_nvidia_block());
        assert!(config.battery_auto_switch());
        assert_eq!(config.battery_auto_switch_mode(), Modes::Smart);
        assert!(config.external_display_auto_switch());
    }

    #[test]
    fn test_cardwire_config_toml_roundtrip() {
        let config = CardwireConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: CardwireConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.auto_apply_gpu_state(), config.auto_apply_gpu_state());
        assert_eq!(
            parsed.experimental_nvidia_block(),
            config.experimental_nvidia_block()
        );
        assert_eq!(parsed.battery_auto_switch(), config.battery_auto_switch());
        assert_eq!(
            parsed.battery_auto_switch_mode(),
            config.battery_auto_switch_mode()
        );
        assert_eq!(
            parsed.external_display_auto_switch(),
            config.external_display_auto_switch()
        );
    }

    #[test]
    fn test_cardwire_config_toml_parse_with_missing_fields_uses_defaults() {
        // Only specify one field — serde(default) should fill the rest
        let toml_str = "auto_apply_gpu_state = false\n";
        let parsed: CardwireConfig = toml::from_str(toml_str).unwrap();
        assert!(!parsed.auto_apply_gpu_state());
        // All others should be defaults
        assert!(!parsed.experimental_nvidia_block());
        assert!(!parsed.battery_auto_switch());
        assert_eq!(parsed.battery_auto_switch_mode(), Modes::Hybrid);
        assert!(!parsed.external_display_auto_switch());
    }

    #[test]
    fn test_cardwire_config_toml_parse_empty_string_uses_all_defaults() {
        let parsed: CardwireConfig = toml::from_str("").unwrap();
        assert!(parsed.auto_apply_gpu_state());
        assert!(!parsed.experimental_nvidia_block());
    }

    #[test]
    fn test_cardwire_config_toml_with_custom_values() {
        let toml_str = r#"
auto_apply_gpu_state = false
experimental_nvidia_block = true
battery_auto_switch = true
battery_auto_switch_mode = "smart"
external_display_auto_switch = true
"#;
        let parsed: CardwireConfig = toml::from_str(toml_str).unwrap();
        assert!(!parsed.auto_apply_gpu_state());
        assert!(parsed.experimental_nvidia_block());
        assert!(parsed.battery_auto_switch());
        assert_eq!(parsed.battery_auto_switch_mode(), Modes::Smart);
        assert!(parsed.external_display_auto_switch());
    }
}
