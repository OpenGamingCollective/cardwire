#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct PciDevice {
    pci_address: String,
    iommu_group: Option<usize>,
    vendor_id: Option<String>,
    device_id: Option<String>,
    vendor_name: Option<String>,
    device_name: Option<String>,
    driver: Option<String>,
    class: Option<String>,
    parent_pci: Option<String>,
    child_pci: Option<String>,
}
impl PciDevice {
    pub fn pci_address(&self) -> &str {
        &self.pci_address
    }

    pub fn iommu_group(&self) -> &Option<usize> {
        &self.iommu_group
    }
    pub fn vendor_id(&self) -> &Option<String> {
        &self.vendor_id
    }
    pub fn device_id(&self) -> &Option<String> {
        &self.device_id
    }
    pub fn vendor_name(&self) -> &Option<String> {
        &self.vendor_name
    }
    pub fn device_name(&self) -> &Option<String> {
        &self.device_name
    }
    pub fn driver(&self) -> &Option<String> {
        &self.driver
    }
    pub fn class(&self) -> &Option<String> {
        &self.class
    }
    pub fn parent_pci(&self) -> &Option<String> {
        &self.parent_pci
    }
    pub fn child_pci(&self) -> &Option<String> {
        &self.child_pci
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pci_address: String,
        iommu_group: Option<usize>,
        vendor_id: Option<String>,
        device_id: Option<String>,
        vendor_name: Option<String>,
        device_name: Option<String>,
        driver: Option<String>,
        class: Option<String>,
        parent_pci: Option<String>,
        child_pci: Option<String>,
    ) -> PciDevice {
        PciDevice {
            pci_address,
            iommu_group,
            vendor_id,
            device_id,
            vendor_name,
            device_name,
            driver,
            class,
            parent_pci,
            child_pci,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct DbusPciDevice {
    pub pci_address: String,
    // Strings to be able to put nothing
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

pub struct IommuGroup {
    pub id: usize,
    pub devices: Vec<String>,
}
