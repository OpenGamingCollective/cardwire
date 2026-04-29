#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct PciDevice {
    pub pci_address: String,
    pub iommu_group: Option<usize>,
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    pub vendor_name: Option<String>,
    pub device_name: Option<String>,
    pub driver: Option<String>,
    pub class: Option<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct DbusPciDevice {
    pub pci_address: String,
    // Strings to be able to put Null
    pub iommu_group: String,
    pub vendor_id: String,
    pub device_id: String,
    pub vendor_name: String,
    pub device_name: String,
    pub driver: String,
    pub class: String,
}

pub struct IommuGroup {
    pub id: usize,
    pub devices: Vec<String>,
}
