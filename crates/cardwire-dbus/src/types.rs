use cardwire_core::{
    app_metadata::types::AppMetadata, gpu::models::{GpuDevice, GpuType}, pci::models::PciDevice
};
use zbus::zvariant;

/// The GpuDevice communicated over D-Bus
#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug)]
pub struct DbusGpuDevice {
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub device_type: GpuType,
    pub vendor: String,
    pub driver: String,
    pub nvidia_minor: String,
}

impl From<&GpuDevice> for DbusGpuDevice {
    /// turn a GpuDevice into one sendable over D-Bus
    fn from(gpu: &GpuDevice) -> Self {
        DbusGpuDevice {
            pci: gpu.pci.pci_address().to_string(),
            render: *gpu.render(),
            name: gpu.name().to_string(),
            card: *gpu.card(),
            default: gpu.is_default(),
            device_type: gpu.device_type().clone(),
            vendor: gpu.gpu_vendor().to_string(),
            driver: gpu.pci.driver().clone().unwrap_or("none".to_string()),
            nvidia_minor: if let Some(minor) = gpu.nvidia_minor() {
                minor.to_string()
            } else {
                "none".to_string()
            },
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct DbusPciDevice {
    // Strings to be able to put nothing
    // TODO: Maybe use a dict/hashmap instead of a struct ?
    pub iommu_group: String,
    pub vendor_id: String,
    pub device_id: String,
    pub vendor_name: String,
    pub device_name: String,
    pub driver: String,
    pub class: String,
    pub parent_pci: String,
    pub child_pci: String,
}

impl From<&PciDevice> for DbusPciDevice {
    /// turn a PciDevice into one sendable over D-Bus
    fn from(pci: &PciDevice) -> Self {
        DbusPciDevice {
            iommu_group: if let Some(iommu) = pci.iommu_group() {
                iommu.to_string()
            } else {
                String::new()
            },
            vendor_id: pci.vendor_id().clone().unwrap_or_default(),
            device_id: pci.device_id().clone().unwrap_or_default(),
            vendor_name: pci.vendor_name().clone().unwrap_or_default(),
            device_name: pci.device_name().clone().unwrap_or_default(),
            driver: pci.driver().clone().unwrap_or_default(),
            class: pci.class().clone().unwrap_or_default(),
            parent_pci: pci.parent_pci().clone().unwrap_or_default(),
            child_pci: pci.child_pci().clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, zvariant::Type, serde::Serialize, serde::Deserialize)]
pub struct DbusAppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
    pub gpu_policy: u32,
}

impl DbusAppMetadata {
    pub fn from_app_metadata(meta: &AppMetadata, gpu_policy: u32) -> Self {
        Self {
            display_name: meta.display_name.clone(),
            desktop_file_id: meta.desktop_file_id.clone(),
            icon_name: meta.icon_name.clone(),
            gpu_policy,
        }
    }
}
