#[derive(Clone)]
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

pub struct IommuGroup {
    pub id: usize,
    pub devices: Vec<String>,
}
