use libdrm_amdgpu_sys::{
    AMDGPU::{DeviceHandle, GPU_INFO}, LibDrmAmdgpu
};

use crate::core::gpu::models::GpuType;
use std::{fs, path::Path};

/// Find the amd model using amdgpu.ids, require the device id and the revision for precise matching
#[allow(unused, dead_code)]
pub fn amd_get_device_name(device_id: &str, pci: &str) -> Option<String> {
    let path = "/usr/share/libdrm/amdgpu.ids";
    let device_id = device_id.to_string().replace("0x", "").to_ascii_uppercase();

    let revision = fs::read_to_string(format!("/sys/bus/pci/devices/{}/revision", pci))
        .ok()?
        .trim()
        .replace("0x", "")
        .to_ascii_uppercase();

    let content = fs::read_to_string(path).ok()?;

    for line in content.lines() {
        if line.starts_with('#') {
            continue;
        }

        let mut parts = line.split('\t');
        let Some(id) = parts.next() else {
            continue;
        };
        let Some(rev) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };

        if id.trim_end_matches(',') == device_id && rev.trim_end_matches(',') == revision {
            return Some(name.to_string());
        }
    }

    None
}

pub struct AmdGpuDev {
    amdgpu_dev: DeviceHandle,
}
impl AmdGpuDev {
    pub fn new(render: u32) -> Self {
        let libdrm_amdgpu = LibDrmAmdgpu::new().unwrap();
        let (amdgpu_dev, _drm_major, _drm_minor) = {
            use std::fs::OpenOptions;
            let path = format!("/dev/dri/renderD{}", render);
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap();

            libdrm_amdgpu.init_device_handle_with_fd(f).unwrap()
        };
        Self { amdgpu_dev }
    }

    /// Get the AMD gpu type using amdgpu_gpu_info
    pub fn amd_get_device_type(&self) -> GpuType {
        const AMDGPU_IDS_FLAGS_FUSION: u64 = 0x01;
        let gpu_info = self.amdgpu_dev.query_gpu_info().unwrap();
        let fusion = gpu_info.ids_flags & AMDGPU_IDS_FLAGS_FUSION;
        if fusion == 0 {
            GpuType::Discrete
        } else {
            GpuType::Integrated
        }
    }

    pub fn amd_get_device_name(&self) -> String {
        let gpu_info = self.amdgpu_dev.device_info().unwrap();
        gpu_info.find_device_name_or_default()
    }
}
