mod discover;
mod ebpf;
mod models;

pub use discover::{check_default_drm_class, read_gpu};
pub use ebpf::{GpuBlocker, block_gpu, is_gpu_blocked};
pub use models::{DbusGpuDevice, GpuDevice};
