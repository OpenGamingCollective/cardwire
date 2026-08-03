mod discover;
mod models;

pub use discover::{check_default_drm_class, external_display_connected, read_gpu};
pub use models::{DbusGpuDevice, GpuDevice, GpuVendor, PowerState};
