use std::{collections::BTreeMap, io};

use log::{error, info, warn};

use crate::core::{
    gpu::{
        GpuDevice, GpuVendor, check_default_drm_class, device_info::nvidia_get_minor, display::drm_node_ids, models::GpuType, type_detection::vulkan::Vulkan
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

    /// Take a pci device and build a GpuDevice
    fn build_gpu(&self, device: &PciDevice) -> io::Result<GpuDevice> {
        let gpu_vendor = match device.vendor_id() {
            Some(id) => GpuVendor::from(id.as_str()),
            // Default to "Other"
            None => GpuVendor::default(),
        };

        // Check if the gpu info can be fetched using vulkan, if so use vulkan to build the GPU
        if self.vulkan.vulkan_compatible(device.pci_address()) {
            let pci_id = device.pci_address();
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

        // Try with vulkan first
        //let device_name = (|| match gpu_vendor {
        //        // Use the driver info
        //        GpuVendor::Nvidia => nvidia_get_device_model(device.pci_address()),
        //        // use amdgpu.ids
        //        GpuVendor::Amd => device
        //            .device_id()
        //            .as_ref()
        //            .and_then(|id| amd_get_device_model(id, device.pci_address())),
        //        _ => None,
        //    })
        //    // Fallback to hwdata
        //    .or_else(|| {
        //        warn!("Couldn't get device_name, falling back to hwdata");
        //        device.device_name().clone()
        //    })
        //    // fallback default
        //    .unwrap_or_else(|| {
        //        warn!("Couldn't get name using hwdata, falling back to default");
        //        "Unknown Device".to_string()
        //    });

        let gpu_name = String::new();

        // If the GPU is bound to vfio, mark it as unavailable
        if let Some(driver) = device.driver()
            && driver.contains("vfio-")
        {
            info!("Device: {} is bound to: {}", gpu_name, driver);
            return Ok(GpuDevice::new(
                gpu_name,
                device.clone(),
                u32::MAX,
                u32::MAX,
                None,
                gpu_vendor,
                None,
                GpuType::Unavailable,
            ));
        }

        let nvidia_minor = match gpu_vendor {
            GpuVendor::Nvidia => nvidia_get_minor(device.pci_address()),
            _ => None,
        };

        // Available is used to know if the device should be used by cardwire or not
        let (card, render, _available) = match drm_node_ids(device.pci_address()) {
            Ok((c, r)) => (c, r, true),
            Err(err) => {
                error!("{}: Couldn't get drm node IDs: {}", gpu_name, err);
                (u32::MAX, u32::MAX, false)
            }
        };
        let device_type = GpuType::Unknown;

        Ok(GpuDevice::new(
            gpu_name,
            device.clone(),
            render,
            card,
            None,
            gpu_vendor,
            nvidia_minor,
            device_type,
        ))
    }
}
