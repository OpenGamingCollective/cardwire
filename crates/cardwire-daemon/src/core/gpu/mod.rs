mod discover;
mod models;

pub use discover::{check_default_drm_class, connected_external_drm_cards, read_gpu};
pub use models::{DbusGpuDevice, GpuDevice, GpuVendor, PowerState};
