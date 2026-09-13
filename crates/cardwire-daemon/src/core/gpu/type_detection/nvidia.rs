use crate::core::gpu::models::GpuType;

#[allow(unused, dead_code)]
pub fn get_nvidia_type(pci_id: &str, gpu_name: &str) -> GpuType {
    GpuType::Unknown
}
