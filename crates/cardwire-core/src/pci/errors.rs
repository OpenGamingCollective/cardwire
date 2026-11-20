use std::{io, path};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PciError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("IOMMU Not Enabled")]
    IommuNotEnabled,

    #[error("Missing 'devices' directory in group path: {0}")]
    MissingDevicesDir(path::PathBuf),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for PciError {
    fn from(value: &str) -> Self {
        PciError::Other(value.to_string())
    }
}

pub type Result<T, E = PciError> = core::result::Result<T, E>;
