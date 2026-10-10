/*
    Laptop = 1 integrated default + 1 discrete/eGPU non default
    Manual = Others
*/

use crate::gpu::models::GpuType;

#[derive(Clone, Debug, PartialEq)]
pub enum SystemType {
    Laptop,
    Manual,
}

impl SystemType {
    /// Get the SystemType from a vec of available gpus (id, default, device_type)
    pub fn from_gpus(gpus: &Vec<(usize, bool, GpuType)>) -> Self {
        // Directly assign system with less or more than 2 GPUs
        if gpus.len() != 2 {
            Self::Manual
        } else if gpus.iter().any(|(_, default, gpu_type)| {
            *gpu_type == GpuType::Discrete || *gpu_type == GpuType::External && !*default
        }) && gpus
            .iter()
            .any(|(_, default, gpu_type)| *gpu_type == GpuType::Integrated && *default)
        {
            // Has a non-default discrete/external GPU and a default integrated GPU
            Self::Laptop
        } else {
            Self::Manual
        }
    }
}
