use std::{collections::BTreeMap, io};

use log::{error, info, warn};

use crate::core::{
    gpu::{
        GpuDevice, GpuVendor, check_default_drm_class, generic::{
            udev::{sysfs_get_device_drm, wait_for_drm}, vulkan::Vulkan
        }, models::GpuType, vendor_specific::{
            amd::AmdGpuDev, intel::intel_get_device_type, nvidia::{nvidia_get_device_minor, nvidia_get_device_name, nvidia_get_device_type}
        }
    }, pci::PciDevice
};

pub struct GpuEnumerator {
    vulkan: Vulkan,
}

impl GpuEnumerator {
    pub fn build() -> Self {
        let vulkan = Vulkan::build();
        Self { vulkan }
    }
    /// Enumerate the GPUS on the host system
    pub fn enumerate(&self, pci_list: &BTreeMap<String, PciDevice>) -> BTreeMap<usize, GpuDevice> {
        let mut gpu_list: BTreeMap<usize, GpuDevice> = BTreeMap::new();

        let mut id = 0;
        for pci_device in pci_list.values().filter(|dev| {
            // Check if the class is tied to graphics
            dev.class()
                .as_ref()
                .is_some_and(|class| class.starts_with("0x03"))
        }) {
            match self.build_gpu(pci_device) {
                Ok(gpu) => {
                    gpu_list.insert(id, gpu);
                    id = id.saturating_add(1);
                }
                Err(err) => {
                    warn!(
                        "Could not initialize GPU for {}: {}",
                        pci_device.pci_address(),
                        err
                    );
                }
            }
        }

        // Check which device is the default
        let _ = check_default_drm_class(&mut gpu_list);

        gpu_list
    }

    /*
       Gpu building:
           first attempt is to use vulkan, this is easier and more precise for type detection, if not available
           use per-vendor + generic methods
    */

    /// Take a pci device and build a GpuDevice
    fn build_gpu(&self, device: &PciDevice) -> io::Result<GpuDevice> {
        let gpu_vendor = match device.vendor_id() {
            Some(id) => GpuVendor::from(id.as_str()),
            // Default to "Other"
            None => GpuVendor::default(),
        };
        let pci_id = device.pci_address();
        // Wait for DRM to be ready, each attempt take 250ms
        let _ = wait_for_drm(pci_id, 5);

        // Check if the gpu info can be fetched using vulkan, if so use vulkan to build the GPU
        if !self.vulkan.vulkan_compatible(device.pci_address()) {
            let gpu_type = self.vulkan.get_gpu_type(pci_id);
            let gpu_name = self.vulkan.get_gpu_name(pci_id);

            let gpu_card = self.vulkan.get_gpu_card(pci_id).unwrap_or_default();
            let gpu_render = self.vulkan.get_gpu_render(pci_id).unwrap_or_default();

            let gpu_device = GpuDevice::new(
                gpu_name,
                device.clone(),
                gpu_render as u32,
                gpu_card as u32,
                None,
                gpu_vendor,
                None,
                gpu_type,
            );
            return Ok(gpu_device);
        }

        // else, fallback to sysfs GPU building

        // For now only support the popular GPU vendors, more can be added inthe future
        match gpu_vendor {
            // We use /proc/driver/nvidia/gpus for nvidia devices
            // TODO: add another match for nova driver
            GpuVendor::Nvidia => {
                // Try to get the device name using nvidia driver, if fail, use hwdata, then
                // fallback to unknown
                let gpu_name = nvidia_get_device_name(pci_id).unwrap_or_else(|| {
                    device
                        .device_name()
                        .clone()
                        .unwrap_or_else(|| "Unknown Device".to_string())
                });
                let drm_res = sysfs_get_device_drm(pci_id);
                // return a working GPU if drm available, else return a non-available GPU
                match drm_res {
                    Some((card, render)) => {
                        let gpu_type = nvidia_get_device_type(pci_id, &gpu_name);
                        let nvidia_minor = nvidia_get_device_minor(pci_id);
                        let gpu_device = GpuDevice::new(
                            gpu_name,
                            device.clone(),
                            render,
                            card,
                            None,
                            gpu_vendor,
                            nvidia_minor,
                            gpu_type,
                        );
                        return Ok(gpu_device);
                    }
                    None => {
                        // Couldn't get DRM, mark GPU as not available
                        let gpu_type = GpuType::Unavailable;
                        let gpu_device = GpuDevice::new(
                            gpu_name,
                            device.clone(),
                            u32::MAX,
                            u32::MAX,
                            None,
                            gpu_vendor,
                            None,
                            gpu_type,
                        );
                        return Ok(gpu_device);
                    }
                }
            }
            GpuVendor::Intel => {
                // Use Hwdata for intel
                let gpu_name = device
                    .device_name()
                    .clone()
                    .unwrap_or_else(|| "Unknown Device".to_string());
                let drm_res = sysfs_get_device_drm(pci_id);
                match drm_res {
                    Some((card, render)) => {
                        let gpu_type = intel_get_device_type(pci_id);
                        let gpu_device = GpuDevice::new(
                            gpu_name,
                            device.clone(),
                            render,
                            card,
                            None,
                            gpu_vendor,
                            None,
                            gpu_type,
                        );
                        return Ok(gpu_device);
                    }
                    None => {
                        // Couldn't get DRM, mark GPU as not available
                        let gpu_type = GpuType::Unavailable;
                        let gpu_device = GpuDevice::new(
                            gpu_name,
                            device.clone(),
                            u32::MAX,
                            u32::MAX,
                            None,
                            gpu_vendor,
                            None,
                            gpu_type,
                        );
                        return Ok(gpu_device);
                    }
                }
            }
            GpuVendor::Amd => {
                // Only support for amdgpu will be added, radeon will be considered on user demand
                if device.driver().clone().is_some_and(|d| d == "amdgpu") {
                    // For AMD, we fetch infos using libdrm_amdgpu if DRM nodes are availables
                    let gpu_name = device
                        .device_name()
                        .clone()
                        .unwrap_or_else(|| "Unknown Device".to_string());
                    let drm_res = sysfs_get_device_drm(pci_id);
                    match drm_res {
                        Some((card, render)) => {
                            let gpu_device = if let Ok(amdgpu) = AmdGpuDev::new(render) {
                                let gpu_type = amdgpu.amd_get_device_type();
                                // amdgpu is more precise
                                let gpu_name = amdgpu.amd_get_device_name();
                                GpuDevice::new(
                                    gpu_name,
                                    device.clone(),
                                    render,
                                    card,
                                    None,
                                    gpu_vendor,
                                    None,
                                    gpu_type,
                                )
                            } else {
                                // If we cannot use AMDGPU, mark device is unknown until i find a
                                // reliable way to detect discrete using sysfs
                                let gpu_type = GpuType::Unknown;
                                GpuDevice::new(
                                    gpu_name,
                                    device.clone(),
                                    render,
                                    card,
                                    None,
                                    gpu_vendor,
                                    None,
                                    gpu_type,
                                )
                            };
                            println!("{}: {:?}", gpu_device.name(), gpu_device.device_type());
                            return Ok(gpu_device);
                        }
                        None => {
                            // Couldn't get DRM, mark GPU as not available
                            let gpu_type = GpuType::Unavailable;
                            let gpu_device = GpuDevice::new(
                                gpu_name,
                                device.clone(),
                                u32::MAX,
                                u32::MAX,
                                None,
                                gpu_vendor,
                                None,
                                gpu_type,
                            );
                            return Ok(gpu_device);
                        }
                    }
                } else {
                    println!("not amdgpu");
                    // Couldn't get DRM, mark GPU as not available
                    let gpu_type = GpuType::Unavailable;
                    let gpu_name = device
                        .device_name()
                        .clone()
                        .unwrap_or_else(|| "Unknown Device".to_string());
                    let gpu_device = GpuDevice::new(
                        gpu_name,
                        device.clone(),
                        u32::MAX,
                        u32::MAX,
                        None,
                        gpu_vendor,
                        None,
                        gpu_type,
                    );
                    return Ok(gpu_device);
                }
            }
            _ => todo!(),
        };
    }
}
