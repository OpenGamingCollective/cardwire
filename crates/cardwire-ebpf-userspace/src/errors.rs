//! custom errors for cardwire-ebpf
use std::{fmt, io};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CardwireEbpfError {
    #[error("LSM not enabled")]
    LSMNotEnabled,
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
    #[error("couldn't load ebpf: {0}")]
    EbpfLoadError(String),
    #[error("missing lsm: {name}")]
    MissingLsm { name: String },
    #[error("missing map: {name}")]
    MissingMap { name: String },
    // for block/unblock, used if passed String is not in a pci format for example
    #[error("wrong format, expected {kind} got: {input}")]
    WrongFormat { kind: String, input: String },
    #[error("{0}")]
    Aya(String),
    #[error("{0}")]
    Other(String),
}

impl CardwireEbpfError {
    pub fn missing_lsm(name: &str) -> Self {
        Self::MissingLsm {
            name: name.to_string(),
        }
    }
    pub fn missing_map(name: &str) -> Self {
        Self::MissingMap {
            name: name.to_string(),
        }
    }
    pub fn aya<E: fmt::Display>(err: E) -> Self {
        Self::Aya(err.to_string())
    }
}

pub type CardwireEbpfResult<T> = Result<T, CardwireEbpfError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_lsm_not_enabled() {
        let err = CardwireEbpfError::LSMNotEnabled;
        assert_eq!(err.to_string(), "LSM not enabled");
    }

    #[test]
    fn test_error_missing_lsm_named() {
        let err = CardwireEbpfError::missing_lsm("file_open");
        assert_eq!(err.to_string(), "missing lsm: file_open");
    }

    #[test]
    fn test_error_missing_map_named() {
        let err = CardwireEbpfError::missing_map("cw_blocked_ino");
        assert_eq!(err.to_string(), "missing map: cw_blocked_ino");
    }

    #[test]
    fn test_error_aya_wraps_message() {
        let err = CardwireEbpfError::aya("some aya error");
        assert_eq!(err.to_string(), "some aya error");
    }

    #[test]
    fn test_error_ebpf_load_error() {
        let err = CardwireEbpfError::EbpfLoadError("failed to load".to_string());
        assert!(err.to_string().contains("failed to load"));
    }

    #[test]
    fn test_error_wrong_format() {
        let err = CardwireEbpfError::WrongFormat {
            kind: "PCI address".to_string(),
            input: "invalid".to_string(),
        };
        assert!(err.to_string().contains("PCI address"));
        assert!(err.to_string().contains("invalid"));
    }

    #[test]
    fn test_error_io_conversion() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let err = CardwireEbpfError::from(io_err);
        assert!(err.to_string().contains("access denied"));
    }

    #[test]
    fn test_error_other() {
        let err = CardwireEbpfError::Other("custom".to_string());
        assert_eq!(err.to_string(), "custom");
    }
}
