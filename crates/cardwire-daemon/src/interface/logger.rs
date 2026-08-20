use std::{
    collections::VecDeque, sync::{Arc, OnceLock}, time::SystemTime
};

use tokio::sync::RwLock;

use zbus::{fdo, interface, object_server::SignalEmitter};

#[derive(Debug, Clone, zbus::zvariant::Type, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub pid: u32,
    pub comm: String,
    pub gpu_id: u32,
    pub wayland_app_id: String,
}

#[derive(Clone)]
pub struct LoggerInterface {
    pub report_logs: Arc<RwLock<VecDeque<LogEntry>>>,
    // Signal emitter for new logs signal, populated once the interface is served
    pub signal_emitter: Arc<OnceLock<SignalEmitter<'static>>>,
}

impl LoggerInterface {
    pub fn build() -> Self {
        Self {
            report_logs: Arc::new(RwLock::new(VecDeque::with_capacity(4096))),
            signal_emitter: Arc::new(OnceLock::new()),
        }
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Logger")]
impl LoggerInterface {
    /// Return a Vec of processes blocked by cardwire
    pub async fn process_blocked(&self) -> fdo::Result<VecDeque<LogEntry>> {
        let vec = self.report_logs.read().await;
        Ok(vec.clone())
    }

    #[zbus(signal)]
    /// Signal when cardwire blocked a process
    pub async fn process_blocked_changed(
        emitter: &SignalEmitter<'_>,
        log: LogEntry,
    ) -> zbus::Result<()>;
}
