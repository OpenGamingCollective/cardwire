use std::{io, path};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("ebpf error: {0}")]
    CardwireEbpfError(#[from] cardwire_ebpf_userspace::CardwireEbpfError),

    #[error("parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    // PCI errors
    #[error("IOMMU Not Enabled")]
    IommuNotEnabled,

    #[error("Missing 'devices' directory in group path: {0}")]
    MissingDevicesDir(path::PathBuf),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_from_str() {
        let err = Error::from("something went wrong");
        match err {
            Error::Other(msg) => assert_eq!(msg, "something went wrong"),
            _ => panic!("expected Error::Other"),
        }
    }

    #[test]
    fn test_error_display_iommu_not_enabled() {
        let err = Error::IommuNotEnabled;
        assert_eq!(err.to_string(), "IOMMU Not Enabled");
    }

    #[test]
    fn test_error_display_missing_devices_dir() {
        let err = Error::MissingDevicesDir(std::path::PathBuf::from("/sys/test"));
        assert!(err.to_string().contains("/sys/test"));
    }

    #[test]
    fn test_error_display_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = Error::Io(io_err);
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_error_display_parse_int() {
        let parse_err = "abc".parse::<u32>().unwrap_err();
        let err = Error::ParseInt(parse_err);
        assert!(err.to_string().contains("parse"));
    }

    #[test]
    fn test_error_display_other() {
        let err = Error::Other("custom error".to_string());
        assert_eq!(err.to_string(), "custom error");
    }
}
