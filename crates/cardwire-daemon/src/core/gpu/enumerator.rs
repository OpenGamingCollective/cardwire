use std::{
    collections::{BTreeMap, HashMap}, io, sync::Arc
};

use log::{error, info, warn};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};

use crate::core::{
    gpu::{
        GpuDevice, GpuVendor, check_default_drm_class, egl::is_discrete_egl, helpers::{amd_get_device_model, drm_node_ids, nvidia_get_device_model, nvidia_get_minor}, vulkan::vlk_enumerate
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
            //  Fallback to vendor-specific lookup
            .or_else(|| match gpu_vendor {
                GpuVendor::Nvidia => nvidia_get_device_model(device.pci_address()),
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
                false,
                true,
                false,
                false,
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

        let discrete = self.is_discrete_vulkan(device.pci_address())
            || match is_discrete_egl(render) {
                Ok(discrete) => discrete,
                Err(err) => {
                    warn!("{}: EGL discrete check failed: {}", device_name, err);
                    false
                }
            };

        Ok(GpuDevice::new(
            device_name,
            device.clone(),
            render,
            card,
            None,
            gpu_vendor,
            nvidia_minor,
            discrete,
            false,
            available,
            self.is_virtual_gpu(device),
        ))
    }
    fn is_discrete_vulkan(&self, pci_id: &str) -> bool {
        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(pci_id)
        {
            return vlk_dev.properties().device_type == PhysicalDeviceType::DiscreteGpu;
        }

        false
    }
    /// Detect virtual GPUs (e.g. virtio-gpu in qemu) through Vulkan when available, falling
    /// back to the virtio PCI vendor id.
    fn is_virtual_gpu(&self, device: &PciDevice) -> bool {
        const VIRTIO_VENDOR_ID: &str = "0x1af4";

        if let Some(vlk_map) = &self.vlk_physical_devices
            && let Some(vlk_dev) = vlk_map.get(device.pci_address())
        {
            return vlk_dev.properties().device_type == PhysicalDeviceType::VirtualGpu;
        }

        device
            .vendor_id()
            .as_deref()
            .is_some_and(|id| id == VIRTIO_VENDOR_ID)
    }
}
