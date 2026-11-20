use zbus::{Result, proxy};

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Manager"
)]
pub trait CardwireManager {
    fn status(&self) -> Result<()>;
}
