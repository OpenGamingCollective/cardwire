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
    #[error("missing {kind}: {name}")]
    MissingEntity { kind: String, name: String },
    #[error("aya error: {0}")]
    Aya(String),
    #[error("{0}")]
    Other(String),
}

impl CardwireEbpfError {
    pub fn missing_entity(kind: &str, name: &str) -> Self {
        Self::MissingEntity {
            kind: kind.to_string(),
            name: name.to_string(),
        }
    }

    pub fn aya<E: fmt::Display>(err: E) -> Self {
        Self::Aya(err.to_string())
    }
}

pub type CardwireEbpfResult<T> = Result<T, CardwireEbpfError>;
