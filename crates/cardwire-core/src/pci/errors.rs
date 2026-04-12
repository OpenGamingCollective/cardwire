use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]

pub enum IommuError {
    #[error("IOMMU Not Enabled")]
    IOMMUNotEnabled,

    #[error("Missing 'devices' directory in group path: {0}")]
    MissingDevicesDir(PathBuf),

    #[error("Missing hwdata pci.ids file")]
    MissingHWData,

    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("{0}")]
    Other(String),
}
impl From<&str> for IommuError {
    fn from(s: &str) -> Self {
        IommuError::Other(s.to_string())
    }
}
