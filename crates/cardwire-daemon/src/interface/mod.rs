mod config;
mod debug;
mod gpu;
mod mode;
mod switcheroo;

pub use config::{ConfigInterface, ConfigMemory};
pub use debug::DebugInterface;
pub use gpu::{GpuInterface, GpuInterfaceSignals};
pub use mode::{ModeInterface, Modes};
pub(crate) use mode::{ModeRuntime, SetModeRequest};
pub use switcheroo::SwitcherooInterface;
