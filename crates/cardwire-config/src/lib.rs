pub mod common;
pub mod config;
pub mod errors;
pub mod sql;
pub mod state;

/// Cardwire configuration directory.
pub const CONFIG_PATH: &str = "/etc/cardwire";

/// Cardwire state directory.
pub const STATE_PATH: &str = "/var/lib/cardwire";