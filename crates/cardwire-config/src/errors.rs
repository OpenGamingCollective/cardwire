use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("rustqlite error: {0}")]
    RusqliteError(#[from] rusqlite::Error),

    #[error("serialize toml error: {0}")]
    TomlSerError(#[from] toml::ser::Error),

    #[error("deserialize toml error: {0}")]
    TomlDeError(#[from] toml::de::Error),

    #[error("Couldn't create the default var folder: {0}")]
    VarFolderError(io::Error),

    #[error("Couldn't generate default config: {0}")]
    DefaultConfigError(toml::ser::Error),

    #[error("Couldn't generate default json state: {0}")]
    DefaultStateError(serde_json::Error),

    #[error("Error with cardwire.toml: {0}")]
    CardwireConfigError(io::Error),

    #[error("Error with state_file {0}: {1}")]
    CardwireStateError(String, serde_json::Error),
}

pub type Result<T, E = ConfigError> = core::result::Result<T, E>;
