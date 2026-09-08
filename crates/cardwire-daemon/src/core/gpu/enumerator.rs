use std::{
    collections::{BTreeMap, HashMap}, io, sync::Arc
};

use log::{error, info, warn};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};

use crate::core::{
    gpu::{
        GpuDevice, GpuVendor, check_default_drm_class, device_info::{amd_get_device_model, nvidia_get_device_model, nvidia_get_minor}, display::drm_node_ids, models::GpuType, vulkan::vlk_enumerate
    }, pci::PciDevice
};

pub struct GpuEnumerator {
    vlk_physical_devices: Option<HashMap<String, Arc<PhysicalDevice>>>,
}

impl GpuEnumerator {
    pub fn build() -> Self {
        // Store the vulkan list to prevent calling vulkan everytime we look into it
        let vlk_physical_devices = vlk_enumerate();

        Self {
            vlk_physical_devices,
        }
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

        // Try with vulkan first
        let device_name = self
            .vlk_physical_devices
            .as_ref()
            .and_then(|map| map.get(device.pci_address()))
            .map(|vlk_dev| vlk_dev.properties().device_name.clone())
            .map(|name| name.split('(').next().unwrap_or(&name).trim().to_string())
            //  Fallback to vendor-specific lookup
            .or_else(|| match gpu_vendor {
                // Use the driver info
                GpuVendor::Nvidia => nvidia_get_device_model(device.pci_address()),
                // use amdgpu.ids
                GpuVendor::Amd => device
                    .device_id()
                    .as_ref()
                    .and_then(|id| amd_get_device_model(id, device.pci_address())),
                _ => None,
            })
            // Fallback to hwdata
            .or_else(|| {
                warn!("Couldn't get device_name, falling back to hwdata");
                device.device_name().clone()
            })
            // fallback default
            .unwrap_or_else(|| {
                warn!("Couldn't get name using hwdata, falling back to default");
                "Unknown Device".to_string()
            });

        // If the GPU is bound to vfio, mark it as unavailable
        if let Some(driver) = device.driver()
            && driver.contains("vfio-")
        {
            info!("Device: {} is bound to: {}", device_name, driver);
            return Ok(GpuDevice::new(
                device_name,
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
        let (card, render, available) = match drm_node_ids(device.pci_address()) {
            Ok((c, r)) => (c, r, true),
            Err(err) => {
                error!("{}: Couldn't get drm node IDs: {}", device_name, err);
                (u32::MAX, u32::MAX, false)
            }
        };

        // Get the device type using vulkan
        let mut device_type = self.get_gpu_type_vulkan(device.pci_address());
        // Mark non-available device
        if !available {
            device_type = GpuType::Unavailable
        };

        Ok(GpuDevice::new(
            device_name,
            device.clone(),
            render,
            card,
            None,
            gpu_vendor,
            nvidia_minor,
            device_type,
        ))
    }
    /// get the gpu type using vulkan
    fn get_gpu_type_vulkan(&self, pci_id: &str) -> GpuType {
        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(pci_id)
        {
            match vlk_dev.properties().device_type {
                PhysicalDeviceType::Cpu => GpuType::Cpu,
                PhysicalDeviceType::DiscreteGpu => GpuType::Discrete,
                PhysicalDeviceType::IntegratedGpu => GpuType::Integrated,
                PhysicalDeviceType::VirtualGpu => GpuType::Virtual,
                PhysicalDeviceType::Other => GpuType::Other,
                _ => {
                    // List is non-exhaustive, warn and give it the unknown type
                    warn!(
                        "{} Unknown GPU type: {:?}",
                        pci_id,
                        vlk_dev.properties().device_type
                    );
                    GpuType::Unknown
                }
            }
        } else {
            GpuType::Unknown
        }
    }
}
