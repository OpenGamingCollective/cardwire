use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyzerError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
}

pub type Result<T, E = AnalyzerError> = std::result::Result<T, E>;
