use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]

pub enum IommuError {
    #[error("IOMMU Not Enabled")]
    IOMMUNotEnabled,

    #[error("Missing 'devices' directory in group path: {0}")]
    MissingDevicesDir(PathBuf),

    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
}
