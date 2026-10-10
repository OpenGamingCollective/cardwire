use zbus::proxy;

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Config"
)]
pub trait CardwireConfig {
    #[zbus(property)]
    pub fn experimental_nvidia_block(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    pub fn auto_apply_gpu_state(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    pub fn battery_auto_switch(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    pub fn battery_auto_switch_mode(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    pub fn external_display_auto_switch(&self) -> zbus::Result<bool>;
}
