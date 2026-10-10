use std::{collections::VecDeque, time::SystemTime};

use zbus::proxy;

#[derive(serde::Deserialize, zbus::zvariant::Type, Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub pid: u32,
    pub comm: String,
    pub gpu_id: u32,
    pub wayland_app_id: String,
}

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Logger"
)]
pub trait CardwireLogger {
    pub fn process_blocked(&self) -> zbus::Result<VecDeque<LogEntry>>;
    #[zbus(signal)]
    pub fn process_blocked_changed(&self, log: LogEntry) -> zbus::Result<()>;
}
