use std::{io, path};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CardwireError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("ebpf error: {0}")]
    CardwireEbpfError(#[from] cardwire_ebpf_userspace::CardwireEbpfError),

    #[error("zbus error: {0}")]
    ZbusError(#[from] zbus::Error),

    #[error("fdo error: {0}")]
    FdoError(#[from] zbus::fdo::Error),

    #[error("rustqlite error: {0}")]
    RusqliteError(#[from] rusqlite::Error),

    #[error("parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    // PCI errors
    #[error("IOMMU Not Enabled")]
    IommuNotEnabled,

    #[error("Missing 'devices' directory in group path: {0}")]
    MissingDevicesDir(path::PathBuf),

    // Config Error
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

    // Mode errors
    #[error("unknown mode: {0}")]
    UnknownMode(u32),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for CardwireError {
    fn from(s: &str) -> Self {
        CardwireError::Other(s.to_string())
    }
}
pub type Result<T, E = CardwireError> = core::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_from_str() {
        let err = CardwireError::from("something went wrong");
        match err {
            CardwireError::Other(msg) => assert_eq!(msg, "something went wrong"),
            _ => panic!("expected Error::Other"),
        }
    }

    #[test]
    fn test_error_display_iommu_not_enabled() {
        let err = CardwireError::IommuNotEnabled;
        assert_eq!(err.to_string(), "IOMMU Not Enabled");
    }

    #[test]
    fn test_error_display_missing_devices_dir() {
        let err = CardwireError::MissingDevicesDir(std::path::PathBuf::from("/sys/test"));
        assert!(err.to_string().contains("/sys/test"));
    }

    #[test]
    fn test_error_display_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = CardwireError::Io(io_err);
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_error_display_parse_int() {
        let parse_err = "abc".parse::<u32>().unwrap_err();
        let err = CardwireError::ParseInt(parse_err);
        assert!(err.to_string().contains("parse"));
    }

    #[test]
    fn test_error_display_other() {
        let err = CardwireError::Other("custom error".to_string());
        assert_eq!(err.to_string(), "custom error");
    }
}
