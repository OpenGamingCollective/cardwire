use std::{collections::HashMap, sync::Arc};

use log::warn;
use vulkano::{
    VulkanLibrary, device::physical::{PhysicalDevice, PhysicalDeviceType}, instance::{Instance, InstanceCreateFlags, InstanceCreateInfo}
};

use crate::core::gpu::models::GpuType;

pub struct Vulkan {
    vlk_physical_devices: Option<HashMap<String, Arc<PhysicalDevice>>>,
}

impl Vulkan {
    pub fn build() -> Self {
        Self {
            vlk_physical_devices: vlk_enumerate(),
        }
    }

    /// Verify if the device pci id is in the vulkan enum map
    pub fn vulkan_compatible(&self, pci_id: &str) -> bool {
        self.vlk_physical_devices
            .as_ref()
            .is_some_and(|map| map.contains_key(pci_id))
    }

    /// get the gpu type using vulkan
    pub fn get_gpu_type(&self, pci_id: &str) -> GpuType {
        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(pci_id)
        {
            match vlk_dev.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => GpuType::Discrete,
                PhysicalDeviceType::IntegratedGpu => GpuType::Integrated,
                PhysicalDeviceType::VirtualGpu => GpuType::Virtual,
                PhysicalDeviceType::Other | PhysicalDeviceType::Cpu => GpuType::Other,
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
    /// Get the gpu name using vulkan
    pub fn get_gpu_name(&self, pci_id: &str) -> String {
        let mut name = String::new();
        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(pci_id)
        {
            name = vlk_dev.properties().device_name.clone();
        }
        name.split('(').next().unwrap_or(&name).trim().to_string()
    }
    /// Get the gpu render node using vulkan
    pub fn get_gpu_render(&self, pci_id: &str) -> Option<i64> {
        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(pci_id)
        {
            return vlk_dev.properties().render_minor;
        }
        None
    }
    /// Get the gpu card node using vulkan
    pub fn get_gpu_card(&self, pci_id: &str) -> Option<i64> {
        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(pci_id)
        {
            return vlk_dev.properties().primary_minor;
        }
        None
    }
}
/// enumerate vulkan physical devices, return None if an error happened
fn vlk_enumerate() -> Option<HashMap<String, Arc<PhysicalDevice>>> {
    let library = match VulkanLibrary::new() {
        Ok(lib) => lib,
        Err(err) => {
            warn!("Couldn't find Vulkan library/DLL: {}", err);
            return None;
        }
    };
    let instance = match Instance::new(
        library,
        InstanceCreateInfo {
            flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
            ..Default::default()
        },
    ) {
        Ok(inst) => inst,
        Err(err) => {
            warn!("Could not create Vulkan Instance: {}", err);
            return None;
        }
    };

    let physical_devices_enum = match instance.enumerate_physical_devices() {
        Ok(vlk_enum) => vlk_enum,
        Err(err) => {
            warn!("Could not enumerate vulkan physical devices: {}", err);
            return None;
        }
    };
    let mut vlk_physical_devices: HashMap<String, Arc<PhysicalDevice>> = HashMap::new();

    for vlk_device in physical_devices_enum {
        match (
            vlk_device.properties().pci_domain,
            vlk_device.properties().pci_bus,
            vlk_device.properties().pci_device,
            vlk_device.properties().pci_function,
        ) {
            (Some(domain), Some(bus), Some(device), Some(function)) => {
                let pci_id = format!("{:04x}:{:02x}:{:02x}.{:x}", domain, bus, device, function);
                vlk_physical_devices.insert(pci_id, Arc::clone(&vlk_device));
            }
            _ => {
                warn!(
                    "{}: Not available (VK_EXT_pci_bus_info not supported)",
                    vlk_device.properties().device_name
                );
                continue;
            }
        }
    }
    Some(vlk_physical_devices)
}
