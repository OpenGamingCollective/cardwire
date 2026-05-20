//! custom errors for cardwire-ebpf
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
    #[error("missing lsm: {name}")]
    MissingLsm { name: String },
    #[error("missing map: {name}")]
    MissingMap { name: String },
    // for block/unblock, used if passed String is not in a pci format for example
    #[error("wrong format, expected {kind} got: {input}")]
    WrongFormat { kind: String, input: String },
    #[error("{0}")]
    Aya(String),
    #[error("{0}")]
    Other(String),
}

impl CardwireEbpfError {
    pub fn missing_lsm(name: &str) -> Self {
        Self::MissingLsm {
            name: name.to_string(),
        }
    }
    pub fn missing_map(name: &str) -> Self {
        Self::MissingMap {
            name: name.to_string(),
        }
    }
    pub fn aya<E: fmt::Display>(err: E) -> Self {
        Self::Aya(err.to_string())
    }
}

pub type CardwireEbpfResult<T> = Result<T, CardwireEbpfError>;
