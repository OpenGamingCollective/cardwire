use std::{collections::BTreeMap, io};

use log::{debug, error, info, warn};

use crate::core::{
    gpu::{
        GpuDevice, GpuVendor, check_default_drm_class, generic::{
            udev::{sysfs_get_device_drm, wait_for_drm}, vulkan::Vulkan
        }, models::GpuType, vendor_specific::{
            amd::AmdGpuDev, intel::intel_get_device_type, nvidia::{
                nvidia_get_device_minor, nvidia_get_device_minor_nvml, nvidia_get_device_name, nvidia_get_device_name_nvml, nvidia_get_device_type, nvidia_get_device_type_nvml, wait_for_nvidia
            }
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

        // For cardwire CI, make GPU 0 integrated, and GPU 1 discrete
        if std::env::var_os("CARDWIRE_TESTING").is_some() {
            info!("CARDWIRE TESTING DETECTED");
            // panic if any of them is missing
            let gpu0 = gpu_list.get_mut(&0).unwrap();
            gpu0.set_type(GpuType::Integrated);
            let gpu1 = gpu_list.get_mut(&1).unwrap();
            gpu1.set_type(GpuType::Discrete);
        }

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
        let _ = wait_for_drm(pci_id, 15);

        // Check if the gpu info can be fetched using vulkan, if so use vulkan to build the GPU
        if self.vulkan.vulkan_compatible(pci_id) {
            let gpu_type = self.vulkan.get_gpu_type(pci_id);
            let gpu_name = self.vulkan.get_gpu_name(pci_id);

            match (
                self.vulkan.get_gpu_card(pci_id),
                self.vulkan.get_gpu_render(pci_id),
            ) {
                (Some(card), Some(render)) => {
                    let gpu_device = GpuDevice::new(
                        gpu_name,
                        device.clone(),
                        render as u32,
                        card as u32,
                        None,
                        gpu_vendor,
                        None,
                        gpu_type,
                    );
                    info!("{}: Used Vulkan to build", gpu_device.name());
                    debug!("{:?}", gpu_device);
                    return Ok(gpu_device);
                }
                // Fallback to vendor match
                _ => {}
            }
        }

        // else, fallback to sysfs GPU building

        // For now only support the popular GPU vendors, more can be added inthe future
        match gpu_vendor {
            // TODO: add another match for nova driver
            GpuVendor::Nvidia => {
                // Proprietary nvidia driver, supported by cardwire
                if device.driver().clone().is_some_and(|d| d == "nvidia") {
                    /*
                       This part may sound confusing
                       We first try to use NVML to build the GPU, using NVML allows us to have a good discrete detection
                       If NVML fails/GPU wasnt ready after 5 retries, fallback to the manual method that reads /proc/driver/nvidia
                    */
                    // Wait for the driver to be ready using NVML
                    if let Some(nvml) = wait_for_nvidia(pci_id, 5) {
                        // The device is ready and nvml is available, use it for GPU construction
                        let gpu_name =
                            nvidia_get_device_name_nvml(&nvml, pci_id).unwrap_or_else(|| {
                                device
                                    .device_name()
                                    .clone()
                                    .unwrap_or_else(|| "Unknown Device".to_string())
                            });
                        let gpu_type = nvidia_get_device_type_nvml(&nvml, pci_id);
                        let nvidia_minor = nvidia_get_device_minor_nvml(&nvml, pci_id);
                        // return a working GPU if drm available
                        if let Some((card, render)) = sysfs_get_device_drm(pci_id) {
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
                            info!("{}: Used Nvidia+NVML to build", gpu_device.name());
                            debug!("{:?}", gpu_device);
                            return Ok(gpu_device);
                        }
                    }

                    // If we are here, it means the NVML failed, this is odd, but will still try
                    // using the good old sysfs + /proc
                    // Try to get the device name using nvidia driver, if fail, use hwdata, then
                    // fallback to unknown
                    let gpu_name = nvidia_get_device_name(pci_id).unwrap_or_else(|| {
                        device
                            .device_name()
                            .clone()
                            .unwrap_or_else(|| "Unknown Device".to_string())
                    });

                    // return a working GPU if drm available, else return a non-available GPU
                    // The type detection for this one is kinda dirty, TODO: find a better way
                    if let Some((card, render)) = sysfs_get_device_drm(pci_id) {
                        let gpu_type = nvidia_get_device_type(&gpu_name);
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
                        info!("{}: Used Nvidia+SysFS to build", gpu_device.name());
                        debug!("{:?}", gpu_device);
                        return Ok(gpu_device);
                    }
                } else {
                    // Not a driver we support (eg. nova), will be marked as not available
                    error!(
                        "{}: driver {:?} is not supported by Cardwire, please request it on Github",
                        device.pci_address(),
                        device.driver()
                    );
                }
            }
            GpuVendor::Intel => {
                // I think i915 and Xe should work the same
                let gpu_name = device
                    .device_name()
                    .clone()
                    .unwrap_or_else(|| "Unknown Device".to_string());
                if let Some((card, render)) = sysfs_get_device_drm(pci_id) {
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
                    info!("{}: Used Intel to build", gpu_device.name());
                    debug!("{:?}", gpu_device);
                    return Ok(gpu_device);
                }
                // DRM couldn't be fetched
                error!(
                    "{}: Cannot fetch DRM nodes, marking as un-available",
                    device.pci_address()
                );
            }
            GpuVendor::Amd => {
                // Only support for amdgpu will be added, radeon will be considered on user demand
                if device.driver().clone().is_some_and(|d| d == "amdgpu") {
                    // For AMD, we fetch infos using libdrm_amdgpu if DRM nodes are availables
                    if let Some((card, render)) = sysfs_get_device_drm(pci_id)
                        && let Ok(amdgpu) = AmdGpuDev::new(render)
                    {
                        let gpu_type = amdgpu.amd_get_device_type();
                        let gpu_name = amdgpu.amd_get_device_name();
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
                        info!("{}: Used AMDGPU to build", gpu_device.name());
                        debug!("{:?}", gpu_device);
                        return Ok(gpu_device);
                    }
                    // DRM couldn't be fetched or AMDGPU ioctl error
                    error!(
                        "{}: Cannot fetch DRM nodes or amdgpu ioctl error, marking as un-available",
                        device.pci_address()
                    );
                } else {
                    // Not a driver we support (eg. radeon), will be marked as not available
                    error!(
                        "{}: driver {:?} is not supported by Cardwire, please request it on Github",
                        device.pci_address(),
                        device.driver()
                    );
                }
            }
            // Just set the type to Virtual
            GpuVendor::Virtio => {
                let gpu_name = device
                    .device_name()
                    .clone()
                    .unwrap_or_else(|| "Unknown Device".to_string());
                if let Some((card, render)) = sysfs_get_device_drm(pci_id) {
                    let gpu_type = GpuType::Virtual;
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
                    info!("{}: Used Virtio to build", gpu_device.name());
                    debug!("{:?}", gpu_device);
                    return Ok(gpu_device);
                }
            }
            // Cardwire depends on knowing the GPU type for the modes, mark Other devices as
            // unknown, leaving only hybrid and manual available until support added
            GpuVendor::Other => {
                let gpu_name = device
                    .device_name()
                    .clone()
                    .unwrap_or_else(|| "Unknown Device".to_string());
                if let Some((card, render)) = sysfs_get_device_drm(pci_id) {
                    let gpu_type = GpuType::Unknown;
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
                    warn!(
                        "{}: unknown device vendor ({:?}/{:?}), please request support for it on Github",
                        gpu_device.name(),
                        device.vendor_id(),
                        device.vendor_name()
                    );
                    debug!("{:?}", gpu_device);
                    return Ok(gpu_device);
                }
            }
        }
        // If we are here, an error happend (mostly DRM or libraries), build an un-available GPU
        let gpu_name = device
            .device_name()
            .clone()
            .unwrap_or_else(|| "Unknown Device".to_string());
        Ok(GpuDevice::new(
            gpu_name,
            device.clone(),
            u32::MAX,
            u32::MAX,
            None,
            gpu_vendor,
            None,
            GpuType::Unavailable,
        ))
    }
}
