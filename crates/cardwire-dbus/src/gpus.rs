use std::collections::HashMap;

use zbus::{
    names::OwnedInterfaceName, proxy, zvariant::{OwnedObjectPath, OwnedValue}
};

// This one is to listen to dbus gpu's object
// daemon doesn't send signal on gpu_refresh event, will be implemented later
// for now this one implement power_state and block

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.freedesktop.DBus.ObjectManager"
)]
trait CardwireGpuInt {
    #[allow(clippy::type_complexity)]
    fn get_managed_objects(
        &self,
    ) -> zbus::Result<
        HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>,
    >;
}
