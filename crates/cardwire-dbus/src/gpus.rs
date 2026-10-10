use std::collections::HashMap;

use zbus::{
    Result, names::OwnedInterfaceName, proxy, zvariant::{OwnedObjectPath, OwnedValue}
};

use crate::types::DbusGpuDevice;

// This one is to listen to dbus gpu's object
// daemon doesn't send signal on gpu_refresh event, will be implemented later
// for now this one implement power_state and block

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.freedesktop.DBus.ObjectManager"
)]
pub trait CardwireGpuInt {
    #[allow(clippy::type_complexity)]
    fn get_managed_objects(
        &self,
    ) -> zbus::Result<
        HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>,
    >;
}

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Gpu"
)]
pub trait CardwireGpu {
    fn get_device(&self) -> Result<DbusGpuDevice>;

    #[zbus(property)]
    fn block(&self) -> Result<bool>;

    fn set_block(&self, state: bool) -> Result<()>;

    fn lsof(&self) -> Result<HashMap<String, Vec<String>>>;

    #[zbus(signal)]
    fn power_state_changed(&self, new_power_state: String) -> Result<()>;

    fn power_state(&self) -> Result<String>;

    #[zbus(property)]
    fn env(&self) -> Result<Vec<String>>;

    #[zbus(property)]
    fn launchable(&self) -> Result<bool>;
}
