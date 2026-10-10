use cardwire_core::modes::types::Modes;
use zbus::proxy;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Mode"
)]
pub trait CardwireMode {
    #[zbus(property)]
    fn mode(&self) -> zbus::Result<Modes>;

    fn set_mode(&self, mode: Modes) -> zbus::Result<()>;

    fn available_modes(&self) -> zbus::Result<Vec<Modes>>;
}
