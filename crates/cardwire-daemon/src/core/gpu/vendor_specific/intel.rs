use crate::core::gpu::models::GpuType;

/// Get the gpu type for an intel GPU
#[allow(unused, dead_code)]
pub fn intel_get_device_type(pci_id: &str) -> GpuType {
    // PCI id reserved for iGPUs
    if pci_id == "0000:00:02.0" {
        GpuType::Integrated
    } else {
        GpuType::Discrete
    }
}
