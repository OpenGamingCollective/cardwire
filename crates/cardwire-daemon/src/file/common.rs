//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::file::{CardwireConfig, CardwireGpuState, CardwireGpuUnit, CardwireModeState};
use anyhow::{Context, Ok};
use std::{collections::BTreeMap, fs, io};
const CONFIG_PATH: &str = "/etc/cardwire";
const STATE_PATH: &str = "/var/lib/cardwire";

#[allow(dead_code)]
pub enum FileKind {
    Config,
    GpuState,
    ModeState,
    PciState,
}

/// Create all folders cardwire need
pub fn create_default_folder(kind: FileKind) -> anyhow::Result<()> {
    let directory = match kind {
        FileKind::Config => CONFIG_PATH,
        _ => STATE_PATH,
    };
    // fs error that should make the daemon exit
    if let Err(e) = fs::create_dir_all(directory) {
        let _ = match e.kind() {
            io::ErrorKind::PermissionDenied => return Err(e.into()),
            io::ErrorKind::ReadOnlyFilesystem => return Err(e.into()),
            io::ErrorKind::NotADirectory => return Err(e.into()),
            _ => Ok(()),
        };
    }
    Ok(())
}
/// Helper function to create default file, used for all config struct
pub fn create_default_file(kind: FileKind) -> anyhow::Result<()> {
    let result = match kind {
        FileKind::Config => {
            create_default_folder(FileKind::Config)
                .context("could not create default folder for cardwire.toml")?;
            // Default config for cardwire
            // TODO: Move to default trait?
            let default_config = toml::to_string_pretty(&CardwireConfig::default())?;
            // write
            fs::write(format!("{}/cardwire.toml", CONFIG_PATH), default_config)
        }
        FileKind::GpuState => {
            create_default_folder(FileKind::GpuState)
                .context("could not create default folder for gpu_state.json")?;
            // Default gpu_state for cardwire
            let mut gpu_hash: BTreeMap<String, CardwireGpuUnit> = BTreeMap::new();
            gpu_hash.insert("Null".to_string(), CardwireGpuUnit::default());
            let default_gpu_state = serde_json::to_string_pretty(&gpu_hash)?;
            // write
            fs::write(format!("{}/gpu_state.json", STATE_PATH), default_gpu_state)
        }
        FileKind::ModeState => {
            create_default_folder(FileKind::ModeState)
                .context("could not create default folder for mode.json")?;
            // Default mode for cardwire
            // TODO: Move to default trait?
            let default_state = CardwireModeState::default();
            let default_mode_state = serde_json::to_string_pretty(&default_state)?;
            // write
            fs::write(format!("{}/mode.json", STATE_PATH), default_mode_state)
        }
        FileKind::PciState => {
            create_default_folder(FileKind::PciState)
                .context("could not create default folder for pci_state.json")?;
            // Default pci_state for cardwire, not implemented yet
            // TODO: Move to default trait?
            let default_state = r#"{}"#;
            fs::write(format!("{}/pci_state.json", STATE_PATH), default_state)
        }
    };
    // Handle the fs error here
    let result: anyhow::Result<()> = match result {
        std::result::Result::Ok(()) => Ok(()),
        std::result::Result::Err(e) => match e.kind() {
            io::ErrorKind::PermissionDenied => return Err(e.into()),
            io::ErrorKind::IsADirectory => return Err(e.into()),
            io::ErrorKind::ReadOnlyFilesystem => return Err(e.into()),
            // happen if in: /var/lib/cardwire/gpu_state.json
            // cardwire is a file and not a directory
            io::ErrorKind::NotADirectory => return Err(e.into()),
            // ignore this one
            io::ErrorKind::AlreadyExists => Ok(()),
            // if directory not found, try to create again
            io::ErrorKind::NotFound => create_default_folder(kind),
            _ => Ok(()),
        },
    };
    result
}
