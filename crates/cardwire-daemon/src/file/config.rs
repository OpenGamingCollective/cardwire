//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::{
    Result, core::errors::CardwireError::CardwireConfigError, file::common::{FileKind, create_default_file}, types::Modes
};
use log::warn;
use tokio::io::AsyncWriteExt;

use serde::{Deserialize, Serialize};
use std::{
    fs, io, time::{SystemTime, UNIX_EPOCH}
};

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
    pub fn build() -> Result<CardwireConfig> {
        let config_file = format!("{}/cardwire.toml", crate::CONFIG_PATH);
        // create the config if it doesnt exist
        if !(fs::exists(&config_file)?) {
            Self::create_default_config()?;
        }
        // remove leftover temp files from a save interrupted by a crash
        Self::cleanup_stale_tmp_files();
        // read the config into a string and parse it
        let config_content = fs::read_to_string(&config_file).map_err(CardwireConfigError)?;
        Ok(Self::parse_or_default(&config_content))
    }
    /// Remove leftover cardwire.toml.*.tmp files from a save interrupted by a crash
    fn cleanup_stale_tmp_files() {
        let Ok(entries) = fs::read_dir(crate::CONFIG_PATH) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("cardwire.toml.") && name.ends_with(".tmp") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    /// Parse the .toml content into a CardwireConfig, on parse failure fall back to
    /// defaults instead of taking the daemon down, leaving the broken file untouched
    fn parse_or_default(config_content: &str) -> CardwireConfig {
        match toml::from_str(config_content) {
            Ok(config) => config,
            Err(e) => {
                warn!(
                    "Failed to parse cardwire.toml ({e}); running with default settings, fix the file and restart the daemon"
                );
                CardwireConfig::default()
            }
        }
    }
    /// Create a default cardwire.toml if not present
    fn create_default_config() -> Result<()> {
        create_default_file(FileKind::Config)?;
        Ok(())
    }
    /// Save the config into cardwire.toml, atomically: write to a unique temp file in the same
    /// directory (exclusive create so concurrent saves never share a file), fsync, then rename
    /// over the target so a crash can't truncate the config
    pub async fn save_config(&self) -> io::Result<()> {
        let path = format!("{}/cardwire.toml", crate::CONFIG_PATH);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let tmp_path = format!("{}/cardwire.toml.{}.tmp", crate::CONFIG_PATH, unique);
        let config_toml = match toml::to_string_pretty(&self) {
            Ok(config_toml) => config_toml,
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        };
        let result = async {
            let mut tmp_file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)
                .await?;
            tmp_file.write_all(config_toml.as_bytes()).await?;
            tmp_file.sync_all().await?;
            drop(tmp_file);
            tokio::fs::rename(&tmp_path, &path).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&tmp_path).await;
        }
        result
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

    #[test]
    fn test_cardwire_config_parse_or_default_on_valid_toml() {
        let config = CardwireConfig::parse_or_default(
            "auto_apply_gpu_state = false\nbattery_auto_switch_mode = \"smart\"\n",
        );
        assert!(!config.auto_apply_gpu_state());
        assert_eq!(config.battery_auto_switch_mode(), Modes::Smart);
        assert!(!config.experimental_nvidia_block());
        assert!(!config.external_display_auto_switch());
    }

    #[test]
    fn test_cardwire_config_parse_or_default_on_invalid_toml_uses_defaults() {
        let config = CardwireConfig::parse_or_default("this is not [[[ valid toml");
        assert!(config.auto_apply_gpu_state());
        assert!(!config.experimental_nvidia_block());
        assert!(!config.battery_auto_switch());
        assert_eq!(config.battery_auto_switch_mode(), Modes::Hybrid);
        assert!(!config.external_display_auto_switch());
    }
}
