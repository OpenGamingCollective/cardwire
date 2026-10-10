use std::io;

use cardwire_analyzer::{errors::AnalyzerError, types::AppMetadata};
use cardwire_ebpf_userspace::CardwireEbpfError;
use thiserror::Error;
use tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError};

#[derive(Error, Debug)]
pub enum SmartAnalyzerError {
    #[error("DB worked dropped the reply for process {1}: {0}")]
    DbReceiverError(RecvError, String),

    #[error("Couldn't send new app to DB: {0}")]
    DbSendError(SendError<(String, AppMetadata, tokio::sync::oneshot::Sender<bool>)>),

    #[error("eBPF error: {0}")]
    CardwireEbpfError(#[from] CardwireEbpfError),

    #[error("Analyzer error: {0}")]
    AnalyzerError(#[from] AnalyzerError),

    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
}

pub type Result<T, E = SmartAnalyzerError> = std::result::Result<T, E>;
