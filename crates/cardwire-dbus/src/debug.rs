use zbus::proxy;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Debug"
)]
pub trait CardwireDebug {
    pub fn get_pci_devices(&self) -> zbus::Result<BTreeMap<String, PciDevice>>;
}
