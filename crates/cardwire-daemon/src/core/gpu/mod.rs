mod default_gpu;
mod device_info;
mod display;
mod egl;
mod enumerator;
mod models;
mod nvidia;
mod vulkan;

pub use default_gpu::check_default_drm_class;
#[expect(unused_imports)]
pub use display::{UdevAction, external_display_connected, is_gpu_active, send_drm_uevent};
pub use enumerator::GpuEnumerator;
pub use models::{DbusGpuDevice, GpuDevice, GpuVendor, PowerState};
pub use nvidia::restart_nvidia_powerd;
