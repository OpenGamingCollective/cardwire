use std::collections::HashMap;

use zbus::{Result, proxy};

use crate::types::DbusAppMetadata;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.SmartPolicy"
)]
pub trait CardwireSmartPolicy {
    fn get_app_policies(&self) -> Result<HashMap<String, DbusAppMetadata>>;

    fn request_process_access(&self, pid: u32, policy: String, value: u32) -> Result<()>;

    fn get_process_status(&self, pid: u32) -> Result<(String, Option<u32>)>;

    #[zbus(signal)]
    fn new_app_added(&self, new_app: (String, u32)) -> zbus::Result<()>;
}
