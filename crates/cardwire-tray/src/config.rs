use std::{
    fmt, fs::{self, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}
};

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "tray.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayMode {
    Integrated,
    Hybrid,
    Manual,
    Smart,
}

impl TrayMode {
    pub const ALL: [Self; 4] = [Self::Integrated, Self::Hybrid, Self::Manual, Self::Smart];

    pub const fn value(self) -> u32 {
        match self {
            Self::Integrated => 0,
            Self::Hybrid => 1,
            Self::Manual => 2,
            Self::Smart => 3,
        }
    }

    pub const fn from_value(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Integrated),
            1 => Some(Self::Hybrid),
            2 => Some(Self::Manual),
            3 => Some(Self::Smart),
            _ => None,
        }
    }
}

impl fmt::Display for TrayMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Integrated => "Integrated",
            Self::Hybrid => "Hybrid",
            Self::Manual => "Manual",
            Self::Smart => "Smart",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrayConfig {
    pub toggle_from: TrayMode,
    pub toggle_to: TrayMode,
    // Keep configurations written before tray-only startup was added valid.
    #[serde(default)]
    pub start_in_tray: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            toggle_from: TrayMode::Integrated,
            toggle_to: TrayMode::Hybrid,
            start_in_tray: false,
        }
    }
}

impl TrayConfig {
    pub fn load() -> io::Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents).map_err(io::Error::other)?;
        config.validate()
    }

    pub fn save(self) -> io::Result<()> {
        self.save_to(&config_path()?)
    }

    pub fn save_to(self, path: &Path) -> io::Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(&self).map_err(io::Error::other)?;
        // Write beside the destination so rename remains an atomic operation on
        // the same filesystem. A failed write leaves the previous config intact.
        let temporary = path.with_extension(format!("toml.tmp-{}", std::process::id()));
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

    pub const fn next_mode(self, current: TrayMode) -> TrayMode {
        if current.value() == self.toggle_from.value() {
            self.toggle_to
        } else {
            self.toggle_from
        }
    }

    pub fn with_toggle_from(mut self, mode: TrayMode) -> Self {
        let previous = self.toggle_from;
        self.toggle_from = mode;
        // Selecting the other endpoint swaps the pair instead of temporarily
        // producing an invalid configuration with two identical modes.
        if self.toggle_to == mode {
            self.toggle_to = previous;
        }
        self
    }

    pub fn with_toggle_to(mut self, mode: TrayMode) -> Self {
        let previous = self.toggle_to;
        self.toggle_to = mode;
        // Preserve the same distinct-endpoint invariant as `with_toggle_from`.
        if self.toggle_from == mode {
            self.toggle_from = previous;
        }
        self
    }

    pub const fn with_start_in_tray(mut self, start_in_tray: bool) -> Self {
        self.start_in_tray = start_in_tray;
        self
    }

    fn validate(self) -> io::Result<Self> {
        if self.toggle_from == self.toggle_to {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "tray toggle modes must be different",
            ))
        } else {
            Ok(self)
        }
    }
}

pub fn config_path() -> io::Result<PathBuf> {
    xdg::BaseDirectories::with_prefix("cardwire")
        .get_config_home()
        .map(|path| path.join(CONFIG_FILE))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "could not determine config home"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cardwire-tray-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    #[test]
    fn defaults_to_integrated_and_hybrid() {
        assert_eq!(
            TrayConfig::default(),
            TrayConfig {
                toggle_from: TrayMode::Integrated,
                toggle_to: TrayMode::Hybrid,
                start_in_tray: false,
            }
        );
    }

    #[test]
    fn rejects_duplicate_modes() {
        let path = temporary_path("duplicate");
        fs::write(&path, "toggle_from = 'smart'\ntoggle_to = 'smart'\n").unwrap();
        assert_eq!(
            TrayConfig::load_from(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn saves_and_loads_current_schema() {
        let path = temporary_path("roundtrip");
        let expected = TrayConfig {
            toggle_from: TrayMode::Manual,
            toggle_to: TrayMode::Smart,
            start_in_tray: true,
        };
        expected.save_to(&path).unwrap();
        assert_eq!(TrayConfig::load_from(&path).unwrap(), expected);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn selecting_duplicate_endpoint_swaps_the_other_endpoint() {
        let config = TrayConfig::default().with_toggle_from(TrayMode::Hybrid);
        assert_eq!(config.toggle_from, TrayMode::Hybrid);
        assert_eq!(config.toggle_to, TrayMode::Integrated);
    }

    #[test]
    fn chooses_configured_toggle_destination() {
        let config = TrayConfig {
            toggle_from: TrayMode::Smart,
            toggle_to: TrayMode::Manual,
            start_in_tray: false,
        };
        assert_eq!(config.next_mode(TrayMode::Smart), TrayMode::Manual);
        assert_eq!(config.next_mode(TrayMode::Hybrid), TrayMode::Smart);
    }

    #[test]
    fn loads_legacy_schema_with_visible_gui_default() {
        let path = temporary_path("legacy");
        fs::write(&path, "toggle_from = 'integrated'\ntoggle_to = 'hybrid'\n").unwrap();
        assert!(!TrayConfig::load_from(&path).unwrap().start_in_tray);
        fs::remove_file(path).unwrap();
    }
}
