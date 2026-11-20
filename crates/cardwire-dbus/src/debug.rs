use std::collections::BTreeMap;

use zbus::{Result, proxy};

use crate::types::DbusPciDevice;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Debug"
)]
pub trait CardwireDebug {
    fn get_pci_devices(&self) -> zbus::Result<BTreeMap<String, DbusPciDevice>>;

    fn refresh_gpu(&self) -> Result<()>;
}
