use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum FdoError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
}

pub type Result<T, E = FdoError> = std::result::Result<T, E>;
