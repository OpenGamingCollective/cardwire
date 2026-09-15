use std::collections::BTreeMap;

use crate::{dbus::GpuType, display::GpuDevice};

#[derive(Clone, Debug, PartialEq)]
pub enum SystemType {
    Laptop,
    Desktop,
    Manual,
}
impl SystemType {
    pub fn from_gpulist(gpu_list: &BTreeMap<usize, GpuDevice>) -> Self {
        let available_gpus: Vec<(usize, bool, GpuType)> = gpu_list
            .iter()
            .filter(|(_, gpu)| gpu.device_type != GpuType::Unavailable)
            .map(|(id, gpu)| (*id, gpu.default, gpu.device_type.clone()))
            .collect();

        if available_gpus.len() != 2 {
            Self::Manual
        } else if available_gpus
            .iter()
            .any(|(_, default, device_type)| *default && *device_type == GpuType::Discrete)
            && available_gpus
                .iter()
                .any(|(_, default, device_type)| *device_type != GpuType::Discrete && !*default)
        {
            // Has a default discrete GPU and a non-default non-discrete GPU, desktop and manual are
            // pretty much the same, TODO
            Self::Desktop
        } else if available_gpus
            .iter()
            .any(|(_, default, device_type)| *device_type == GpuType::Discrete && !*default)
            && available_gpus
                .iter()
                .any(|(_, default, device_type)| *device_type != GpuType::Discrete && *default)
        {
            // Has a non-default discrete GPU and a default non-discrete GPU
            Self::Laptop
        } else {
            // Even if it's a desktop, we treat it as a Manual if it doesn't have the iGPU
            Self::Manual
        }
    }
}
