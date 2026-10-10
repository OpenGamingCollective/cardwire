use cardwire_core::modes::types::Modes;
use zbus::{Result, proxy};

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Config"
)]
pub trait CardwireConfig {
    #[zbus(property)]
    fn experimental_nvidia_block(&self) -> Result<bool>;

    fn set_experimental_nvidia_block(&self, state: bool) -> Result<()>;

    #[zbus(property)]
    fn auto_apply_gpu_state(&self) -> Result<bool>;

    fn set_auto_apply_gpu_state(&self, state: bool) -> Result<()>;

    #[zbus(property)]
    fn battery_auto_switch(&self) -> Result<bool>;

    fn set_battery_auto_switch(&self, state: bool) -> Result<()>;

    #[zbus(property)]
    fn battery_auto_switch_mode(&self) -> Result<Modes>;

    fn set_battery_auto_switch_mode(&self, mode: Modes) -> Result<()>;

    #[zbus(property)]
    fn external_display_auto_switch(&self) -> Result<bool>;

    fn set_external_display_auto_switch(&self, state: bool) -> Result<()>;
}
