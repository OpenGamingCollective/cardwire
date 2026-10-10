use zbus::proxy;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Mode"
)]
pub trait CardwireMode {
    #[zbus(property)]
    pub fn mode(&self) -> zbus::Result<u32>;
    pub fn available_modes(&self) -> zbus::Result<Vec<Modes>>;
}
