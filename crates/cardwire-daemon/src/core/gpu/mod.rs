mod discover;
mod models;

pub use discover::{check_default_drm_class, read_gpu};
pub use models::{DbusGpuDevice, GpuDevice, GpuVendor, PowerState};
