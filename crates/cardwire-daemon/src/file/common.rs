//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::{
    Result, core::errors::CardwireError, file::{CardwireConfig, CardwireGpuUnit, CardwireModeState}
};
use std::{collections::BTreeMap, fs, io};

#[allow(dead_code)]
pub enum FileKind {
    Config,
    GpuState,
    ModeState,
}

/// Create all folders cardwire need
pub fn create_default_folder(kind: FileKind) -> Result<(), io::Error> {
    let directory = match kind {
        FileKind::Config => crate::CONFIG_PATH,
        _ => crate::STATE_PATH,
    };
    // fs error that should make the daemon exit
    if let Err(e) = fs::create_dir_all(directory) {
        match e.kind() {
            io::ErrorKind::PermissionDenied => return Err(e),
            io::ErrorKind::ReadOnlyFilesystem => return Err(e),
            io::ErrorKind::NotADirectory => return Err(e),
            _ => {}
        };
    }
    Ok(())
}
/// Helper function to create default file, used for all config struct
pub fn create_default_file(kind: FileKind) -> Result<()> {
    let result = match kind {
        FileKind::Config => {
            create_default_folder(FileKind::Config).map_err(CardwireError::VarFolderError)?;
            // Default config for cardwire
            let default_config = toml::to_string_pretty(&CardwireConfig::default())
                .map_err(CardwireError::DefaultConfigError)?;
            // write
            fs::write(
                format!("{}/cardwire.toml", crate::CONFIG_PATH),
                default_config,
            )
        }
        FileKind::GpuState => {
            create_default_folder(FileKind::GpuState).map_err(CardwireError::VarFolderError)?;
            // Default gpu_state for cardwire
            let mut gpu_hash: BTreeMap<String, CardwireGpuUnit> = BTreeMap::new();
            gpu_hash.insert("Null".to_string(), CardwireGpuUnit::default());
            let default_gpu_state = serde_json::to_string_pretty(&gpu_hash)
                .map_err(CardwireError::DefaultStateError)?;
            // write
            fs::write(
                format!("{}/gpu_state.json", crate::STATE_PATH),
                default_gpu_state,
            )
        }
        FileKind::ModeState => {
            create_default_folder(FileKind::ModeState).map_err(CardwireError::VarFolderError)?;
            // Default mode for cardwire
            let default_state = CardwireModeState::default();
            let default_mode_state = serde_json::to_string_pretty(&default_state)
                .map_err(CardwireError::DefaultStateError)?;
            // write
            fs::write(
                format!("{}/mode.json", crate::STATE_PATH),
                default_mode_state,
            )
        }
    };
    // Handle the fs error here
    let result: Result<()> = match result {
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
            io::ErrorKind::NotFound => create_default_folder(kind).map_err(CardwireError::Io),
            _ => Ok(()),
        },
    };
    result
}
