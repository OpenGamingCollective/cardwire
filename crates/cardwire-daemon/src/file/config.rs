use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    fs::{create_dir, read_to_string, write}, io, path::Path
};

use crate::{CONFIG_PATH, Result, types::Modes};

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct CardwireConfig {
    pub global_settings: Settings,
    pub experimental_features: ExperimentalFeatures,
    pub switcheroo_settings: SwitcherooControlShim,
    pub internal_whitelist: InternalWhitelist,
}

impl CardwireConfig {
    pub fn build() -> Result<CardwireConfig> {
        // first check if /etc/cardwire (or other) exit
        let config_path = Path::new(&CONFIG_PATH);

        // create the folder if it doesnt exist
        if !config_path.exists() {
            info!(
                "[CONFIG] {} doesn't exist, creating the directory...",
                CONFIG_PATH
            );
            // propagate the error if this fail
            create_dir(CONFIG_PATH)?;
        }

        let config_file = config_path.join("cardwire.toml");

        // Config doesnt exist, create a new one with default values
        if !config_file.exists() {
            let cardwire_config = CardwireConfig::default();
            info!("[CONFIG]: Creating default cardwire config",);
            let conf_toml = toml::to_string_pretty(&cardwire_config)?;
            // write
            write(&config_file, conf_toml)?;
            info!("[CONFIG]: wrote config to {:?}", config_file);
            Ok(cardwire_config)
        } else {
            // config alr exist, verify if it's the old one that needs to be migrated or not
            let config_content = read_to_string(&config_file)?;
            if config_content.contains("auto_apply_gpu_state") {
                warn!("[CONFIG]: detected old cardwire config, migrating...");
                let cardwire_config = CardwireConfig::default();
                let conf_toml = toml::to_string_pretty(&cardwire_config)?;
                write(&config_file, conf_toml)?;
                info!("[CONFIG]: wrote config to {:?}", config_file);
                return Ok(cardwire_config);
            }
            // config exist and it is not the old one
            toml::from_str::<CardwireConfig>(&config_content).map_err(|err| err.into())
        }
    }
    pub async fn save_config(&self) -> io::Result<()> {
        // ik this error management sucks
        let config_toml = toml::to_string_pretty(self).map_err(|_| io::ErrorKind::InvalidData)?;
        let path = Path::new(CONFIG_PATH).join("cardwire.toml");
        write(&path, config_toml)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub restore_gpu_states: bool,
    pub battery_switch: BatteryAutoSwitch,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            restore_gpu_states: true,
            battery_switch: BatteryAutoSwitch::default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BatteryAutoSwitch {
    pub enabled: bool,
    pub ac_mode: Modes,
    pub bat_mode: Modes,
}

impl Default for BatteryAutoSwitch {
    fn default() -> Self {
        Self {
            enabled: false,
            ac_mode: Modes::Hybrid,
            bat_mode: Modes::Integrated,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExperimentalFeatures {
    pub advanced_nvidia_blocking: bool,
    pub fake_drm_uevent: bool,
}

impl Default for ExperimentalFeatures {
    fn default() -> Self {
        Self {
            // Opt-out for nvidia blocking, it has proven to be essential for laptops
            advanced_nvidia_blocking: true,
            // Still experimental and unstable
            fake_drm_uevent: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SwitcherooControlShim {
    pub enabled: bool,
    pub cardwire_envs: bool,
    pub switcheroo_envs: bool,
}
// for users who only wants cardwire's envs
impl Default for SwitcherooControlShim {
    fn default() -> Self {
        Self {
            enabled: true,
            cardwire_envs: true,
            switcheroo_envs: true,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InternalWhitelist {
    pub packages_managers: bool,
    pub vfio: bool,
    pub systemd: bool,
    pub nvidia_powerd: bool,
}

impl Default for InternalWhitelist {
    fn default() -> Self {
        Self {
            packages_managers: true,
            vfio: true,
            systemd: true,
            // Better than restarting the service everytime, and fix issue with smart mode
            nvidia_powerd: true,
        }
    }
}
