#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type, PartialEq)]
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

impl From<&PciDevice> for DbusPciDevice {
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

#[allow(dead_code)]
pub struct IommuGroup {
    pub id: usize,
    pub devices: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_device_accessors_return_constructed_values() {
        let pci = PciDevice::new(
            "0000:01:00.0".to_string(),
            Some(5),
            Some("0x1002".to_string()),
            Some("0x7480".to_string()),
            Some("AMD".to_string()),
            Some("Navi 31".to_string()),
            Some("amdgpu".to_string()),
            Some("0x030000".to_string()),
            Some("0000:00:01.0".to_string()),
            Some("0000:01:00.1".to_string()),
        );
        assert_eq!(pci.pci_address(), "0000:01:00.0");
        assert_eq!(*pci.iommu_group(), Some(5));
        assert_eq!(pci.vendor_id().as_deref(), Some("0x1002"));
        assert_eq!(pci.device_id().as_deref(), Some("0x7480"));
        assert_eq!(pci.vendor_name().as_deref(), Some("AMD"));
        assert_eq!(pci.device_name().as_deref(), Some("Navi 31"));
        assert_eq!(pci.driver().as_deref(), Some("amdgpu"));
        assert_eq!(pci.class().as_deref(), Some("0x030000"));
        assert_eq!(pci.parent_pci().as_deref(), Some("0000:00:01.0"));
        assert_eq!(pci.child_pci().as_deref(), Some("0000:01:00.1"));
    }

    #[test]
    fn test_pci_device_with_all_none_fields() {
        let pci = PciDevice::new(
            "0000:02:00.0".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(pci.pci_address(), "0000:02:00.0");
        assert_eq!(*pci.iommu_group(), None);
        assert_eq!(pci.vendor_id().as_deref(), None);
        assert_eq!(pci.device_id().as_deref(), None);
        assert_eq!(pci.vendor_name().as_deref(), None);
        assert_eq!(pci.device_name().as_deref(), None);
        assert_eq!(pci.driver().as_deref(), None);
        assert_eq!(pci.class().as_deref(), None);
        assert_eq!(pci.parent_pci().as_deref(), None);
        assert_eq!(pci.child_pci().as_deref(), None);
    }
}
