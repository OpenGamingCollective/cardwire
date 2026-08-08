use std::{
    fs::{self, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}
};

use serde::{Deserialize, Serialize};
use strum::VariantArray;

use crate::models::Mode;

const CONFIG_FILE: &str = "gui.toml";
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, VariantArray)]
#[serde(rename_all = "kebab-case")]
pub enum PrimaryClickAction {
    SwitchMode,
    #[default]
    OpenGui,
}

impl std::fmt::Display for PrimaryClickAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SwitchMode => write!(f, "Switch mode"),
            Self::OpenGui => write!(f, "Open GUI"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuiConfig {
    #[serde(default)]
    pub start_in_tray: bool,
    #[serde(default)]
    pub primary_click_action: PrimaryClickAction,
    #[serde(default = "default_primary_click_modes")]
    pub primary_click_modes: Vec<Mode>,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            start_in_tray: false,
            primary_click_action: PrimaryClickAction::OpenGui,
            primary_click_modes: default_primary_click_modes(),
        }
    }
}

impl GuiConfig {
    pub fn load() -> io::Result<Self> {
        let path = config_path()?;
        if path.exists() {
            Self::load_from(&path)
        } else {
            Ok(Self::default())
        }
    }

    fn load_from(path: &Path) -> io::Result<Self> {
        let config = toml::from_str(&fs::read_to_string(path)?).map_err(io::Error::other)?;
        Self::validate(config)
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&config_path()?)
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        Self::validate(self.clone())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self).map_err(io::Error::other)?;
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("toml.tmp-{}-{sequence}", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temporary)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }

    pub fn next_primary_click_mode(&self, current: Mode, available_modes: &[Mode]) -> Mode {
        let modes = if available_modes.is_empty() {
            Mode::VARIANTS
        } else {
            available_modes
        };
        let current_index = modes.iter().position(|&mode| mode == current);
        current_index
            .into_iter()
            .flat_map(|index| modes.iter().cycle().skip(index + 1).take(modes.len()))
            .copied()
            .find(|mode| self.primary_click_modes.contains(mode))
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| self.primary_click_modes.contains(mode))
            })
            .unwrap_or_else(|| modes[0])
    }

    pub fn with_primary_click_mode(mut self, mode: Mode, enabled: bool) -> Self {
        if enabled {
            if !self.primary_click_modes.contains(&mode) {
                self.primary_click_modes.push(mode);
            }
        } else {
            self.primary_click_modes
                .retain(|&configured| configured != mode);
        }
        self
    }

    fn validate(config: Self) -> io::Result<Self> {
        if config.primary_click_modes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "at least one primary-click mode must be configured",
            ));
        }
        if config
            .primary_click_modes
            .iter()
            .enumerate()
            .any(|(index, mode)| config.primary_click_modes[..index].contains(mode))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "primary-click modes must not contain duplicates",
            ));
        }
        Ok(config)
    }
}

fn default_primary_click_modes() -> Vec<Mode> {
    vec![Mode::Integrated, Mode::Hybrid]
}

fn config_path() -> io::Result<PathBuf> {
    xdg::BaseDirectories::with_prefix("cardwire")
        .get_config_home()
        .map(|path| path.join(CONFIG_FILE))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "could not determine config home"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cardwire-gui-config-{label}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    #[test]
    fn defaults_to_the_existing_primary_click_behavior() {
        assert_eq!(
            GuiConfig::default().primary_click_modes,
            default_primary_click_modes()
        );
        assert_eq!(
            GuiConfig::default().primary_click_action,
            PrimaryClickAction::OpenGui
        );
        assert!(!GuiConfig::default().start_in_tray);
    }

    #[test]
    fn saves_and_loads_current_schema() {
        let path = temporary_path("roundtrip");
        let expected = GuiConfig {
            start_in_tray: true,
            primary_click_action: PrimaryClickAction::OpenGui,
            primary_click_modes: vec![Mode::Manual, Mode::Smart],
        };
        expected.save_to(&path).unwrap();
        assert_eq!(GuiConfig::load_from(&path).unwrap(), expected);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_empty_or_duplicate_mode_lists() {
        let empty = temporary_path("empty");
        fs::write(&empty, "primary_click_modes = []").unwrap();
        assert_eq!(
            GuiConfig::load_from(&empty).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(empty).unwrap();

        let duplicate = temporary_path("duplicate");
        fs::write(&duplicate, "primary_click_modes = ['smart', 'smart']").unwrap();
        assert_eq!(
            GuiConfig::load_from(&duplicate).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(duplicate).unwrap();
    }

    #[test]
    fn rejects_unknown_fields() {
        let path = temporary_path("unknown");
        fs::write(&path, "unknown = true").unwrap();
        assert!(GuiConfig::load_from(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn cycles_enabled_modes_in_fixed_order() {
        let config = GuiConfig {
            primary_click_modes: vec![Mode::Integrated, Mode::Manual, Mode::Smart],
            ..GuiConfig::default()
        };
        assert_eq!(
            config.next_primary_click_mode(Mode::Integrated, &[]),
            Mode::Manual
        );
        assert_eq!(
            config.next_primary_click_mode(Mode::Manual, &[]),
            Mode::Smart
        );
        assert_eq!(
            config.next_primary_click_mode(Mode::Smart, &[]),
            Mode::Integrated
        );
        assert_eq!(
            config.next_primary_click_mode(Mode::Hybrid, &[]),
            Mode::Manual
        );
    }

    #[test]
    fn one_enabled_mode_is_selected_from_any_other_mode() {
        let config = GuiConfig {
            primary_click_modes: vec![Mode::Smart],
            ..GuiConfig::default()
        };
        assert_eq!(
            config.next_primary_click_mode(Mode::Hybrid, &[]),
            Mode::Smart
        );
        assert_eq!(
            config.next_primary_click_mode(Mode::Smart, &[]),
            Mode::Smart
        );
    }

    #[test]
    fn skips_unavailable_configured_modes() {
        let config = GuiConfig {
            primary_click_modes: vec![Mode::Integrated, Mode::Hybrid, Mode::Smart],
            ..GuiConfig::default()
        };
        let available = vec![Mode::Hybrid, Mode::Manual];
        assert_eq!(
            config.next_primary_click_mode(Mode::Hybrid, &available),
            Mode::Hybrid
        );
    }

    #[test]
    fn concurrent_saves_use_distinct_temporary_files() {
        let path = temporary_path("concurrent");
        let configs = [
            GuiConfig {
                start_in_tray: true,
                ..GuiConfig::default()
            },
            GuiConfig::default().with_primary_click_mode(Mode::Smart, true),
        ];
        let writers = configs.clone().map(|config| {
            let path = path.clone();
            std::thread::spawn(move || config.save_to(&path))
        });

        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        let saved = GuiConfig::load_from(&path).unwrap();
        assert!(configs.contains(&saved));
        fs::remove_file(path).unwrap();
    }
}
