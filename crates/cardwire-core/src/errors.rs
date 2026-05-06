use std::{io, path};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("ebpf error: {0}")]
    CardwireEbpfError(#[from] cardwire_ebpf::CardwireEbpfError),

    #[error("parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    // PCI errors
    #[error("IOMMU Not Enabled")]
    IommuNotEnabled,

    #[error("Missing 'devices' directory in group path: {0}")]
    MissingDevicesDir(path::PathBuf),

    #[error("Missing hwdata pci.ids file")]
    MissingHWData,

    // GPU errors
    #[error("couldn't check block state for {0}")]
    UnknownBlockState(String),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}
