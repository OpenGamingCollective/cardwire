mod default_gpu;
mod device_info;
mod display;
mod egl;
mod enumerator;
mod models;
mod nvidia;
mod uevent;
mod vulkan;

pub use default_gpu::check_default_drm_class;
#[expect(unused_imports)]
pub use display::{external_display_connected, is_gpu_active};
pub use enumerator::GpuEnumerator;
pub use models::{DbusGpuDevice, GpuDevice, GpuVendor, PowerState};
pub use nvidia::{start_nvidia_powerd, stop_nvidia_powerd};
pub use uevent::{DrmUEvents, send_uevent_blocking};
