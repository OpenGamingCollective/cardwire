use std::{collections::HashMap, sync::Arc};

use log::warn;
use vulkano::{
    VulkanLibrary, device::physical::PhysicalDevice, instance::{Instance, InstanceCreateFlags, InstanceCreateInfo}
};

/// enumerate vulkan physical devices, return None if an error happened
pub fn vlk_enumerate() -> Option<HashMap<String, Arc<PhysicalDevice>>> {
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
