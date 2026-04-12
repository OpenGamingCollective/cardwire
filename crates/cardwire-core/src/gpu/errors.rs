use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("couldn't check block state for {0}")]
    UnknownBlockState(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("ebpf error: {0}")]
    CardwireEbpfError(#[from] cardwire_ebpf::CardwireEbpfError),
    #[error("parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}
pub type GpuResult<T> = Result<T, GpuError>;
