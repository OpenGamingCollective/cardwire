use std::io;

use nvml_wrapper::error::NvmlError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to query amdgpu info {0}")]
    AmdGpuError(i32),

    #[error("Failed to init Nvml {0}")]
    NvmlError(NvmlError),
}

pub type Result<T, E = GpuError> = core::result::Result<T, E>;
