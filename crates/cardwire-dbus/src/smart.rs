use zbus::proxy;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.SmartPolicy"
)]
pub trait CardwireSmartPolicy {
    pub fn get_app_policies(&self)
    -> zbus::Result<HashMap<String, crate::models::DbusAppMetadata>>;
    #[zbus(signal)]
    pub fn new_app_added(
        &self,
        new_app: (String, crate::models::DbusAppMetadata),
    ) -> zbus::Result<()>;
}
