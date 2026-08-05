use std::{collections::VecDeque, sync::Arc, time::SystemTime};

use tokio::sync::RwLock;

use zbus::{fdo, interface, object_server::SignalEmitter};

#[derive(Debug, Clone, zbus::zvariant::Type, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub pid: u32,
    pub comm: String,
    pub gpu_id: u32,
}

#[derive(Clone)]
pub struct LoggerInterface {
    pub report_logs: Arc<RwLock<VecDeque<LogEntry>>>,
}

impl LoggerInterface {
    pub fn build() -> Self {
        Self {
            report_logs: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Logger")]
impl LoggerInterface {
    pub async fn process_blocked(&self) -> fdo::Result<LogEntry> {
        Err(fdo::Error::AccessDenied("only use signal".to_string()))
    }

    #[zbus(signal)]
    pub async fn process_blocked_changed(
        emitter: &SignalEmitter<'_>,
        log: LogEntry,
    ) -> zbus::Result<()>;
}
