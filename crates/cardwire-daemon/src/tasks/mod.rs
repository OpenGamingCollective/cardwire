mod battery_switch;
mod monitor_display;
mod monitor_udev;
mod watch_power_state;

pub use battery_switch::watch_battery_status;
pub(crate) use monitor_display::detect_external_display_target;
pub use monitor_display::monitor_display_changes;
pub use monitor_udev::monitor_pci_changes;
pub use watch_power_state::watch_power_state;
