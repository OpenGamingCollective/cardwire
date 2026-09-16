mod enumerator;
mod generic;
mod models;
mod vendor_specific;

pub use enumerator::GpuEnumerator;
pub use generic::default_gpu::check_default_drm_class;
#[expect(unused_imports)]
pub use generic::display::{external_display_connected, is_gpu_active};
pub use models::{DbusGpuDevice, GpuDevice, GpuType, GpuVendor, PowerState};
pub use vendor_specific::nvidia::{start_nvidia_powerd, stop_nvidia_powerd};
