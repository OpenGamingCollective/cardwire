use std::{fmt::Display, str::FromStr};

use crate::core::pci::PciDevice;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum PowerState {
    D0,
    D1,
    D2,
    D3Hot,
    D3Cold,
    #[default]
    Unknown,
}
impl FromStr for PowerState {
    type Err = std::io::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "D0" => Ok(PowerState::D0),
            "D1" => Ok(PowerState::D1),
            "D2" => Ok(PowerState::D2),
            "D3hot" => Ok(PowerState::D3Hot),
            "D3cold" => Ok(PowerState::D3Cold),
            _ => Ok(PowerState::Unknown),
        }
    }
}
impl Display for PowerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerState::D0 => write!(f, "D0"),
            PowerState::D1 => write!(f, "D1"),
            PowerState::D2 => write!(f, "D2"),
            PowerState::D3Hot => write!(f, "D3Hot"),
            PowerState::D3Cold => write!(f, "D3Cold"),
            _ => write!(f, "unknown"),
        }
    }
}

#[derive(
    Clone,
    serde::Serialize,
    serde::Deserialize,
    zbus::zvariant::Type,
    PartialEq,
    Copy,
    Debug,
    Default,
)]
pub enum GpuVendor {
    Amd,
    Nvidia,
    Intel,
    #[default]
    Other,
}
impl<T: AsRef<str>> From<T> for GpuVendor {
    fn from(string: T) -> Self {
        let vendor_id = string.as_ref();
        // Match vendor id into the GpuVendor enum,
        // nvidia ids found at <https://envytools.readthedocs.io/en/latest/hw/pciid.html>
        match vendor_id {
            "0x1002" => GpuVendor::Amd,
            "0x10de" | "0x104a" | "0x12d2" => GpuVendor::Nvidia,
            "0x8086" => GpuVendor::Intel,
            // Unknown id
            _ => GpuVendor::Other,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type, PartialEq)]
pub struct GpuDevice {
    name: String,
    pub pci: PciDevice,
    render: u32,
    card: u32,
    default: Option<bool>,
    gpu_vendor: GpuVendor,
    nvidia_minor: Option<u32>,
    discrete: bool,
    vfio: bool,
    available: bool,
    virtual_gpu: bool,
}
impl GpuDevice {
    pub fn pci(&self) -> &PciDevice {
        &self.pci
    }

    pub fn default(&self) -> Option<bool> {
        self.default
    }

    pub fn set_default(&mut self, default: Option<bool>) {
        self.default = default;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render(&self) -> &u32 {
        &self.render
    }

    pub fn card(&self) -> &u32 {
        &self.card
    }
    pub fn gpu_vendor(&self) -> GpuVendor {
        self.gpu_vendor
    }
    pub fn nvidia_minor(&self) -> &Option<u32> {
        &self.nvidia_minor
    }

    pub fn is_discrete(&self) -> bool {
        self.discrete
    }

    pub fn set_discrete(&mut self, discrete: bool) {
        self.discrete = discrete;
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn _vfio(&self) -> bool {
        self.vfio
    }

    /// True for virtual GPUs (e.g. virtio-gpu in qemu) that expose no PCI display controller.
    pub fn is_virtual(&self) -> bool {
        self.virtual_gpu
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        pci: PciDevice,
        render: u32,
        card: u32,
        default: Option<bool>,
        gpu_vendor: GpuVendor,
        nvidia_minor: Option<u32>,
        discrete: bool,
        vfio: bool,
        available: bool,
        virtual_gpu: bool,
    ) -> GpuDevice {
        GpuDevice {
            name,
            pci,
            render,
            card,
            default,
            gpu_vendor,
            nvidia_minor,
            discrete,
            vfio,
            available,
            virtual_gpu,
        }
    }

    pub fn is_default(&self) -> bool {
        self.default.unwrap_or(false)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct DbusGpuDevice {
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pci::PciDevice;

    fn make_pci() -> PciDevice {
        PciDevice::new(
            "0000:01:00.0".to_string(),
            Some(1),
            Some("0x1002".to_string()),
            Some("0x1234".to_string()),
            Some("AMD".to_string()),
            Some("RX 7900".to_string()),
            Some("amdgpu".to_string()),
            Some("0x030000".to_string()),
            None,
            None,
        )
    }

    /*
        PowerState enum
    */

    #[test]
    fn test_power_state_from_str_all_valid_variants() {
        assert_eq!("D0".parse::<PowerState>().unwrap(), PowerState::D0);
        assert_eq!("D1".parse::<PowerState>().unwrap(), PowerState::D1);
        assert_eq!("D2".parse::<PowerState>().unwrap(), PowerState::D2);
        assert_eq!("D3hot".parse::<PowerState>().unwrap(), PowerState::D3Hot);
        assert_eq!("D3cold".parse::<PowerState>().unwrap(), PowerState::D3Cold);
    }

    #[test]
    fn test_power_state_from_str_unknown_input() {
        assert_eq!("D4".parse::<PowerState>().unwrap(), PowerState::Unknown);
        assert_eq!("".parse::<PowerState>().unwrap(), PowerState::Unknown);
        assert_eq!(
            "garbage".parse::<PowerState>().unwrap(),
            PowerState::Unknown
        );
    }

    #[test]
    fn test_power_state_display_roundtrip() {
        assert_eq!(PowerState::D0.to_string(), "D0");
        assert_eq!(PowerState::D1.to_string(), "D1");
        assert_eq!(PowerState::D2.to_string(), "D2");
        assert_eq!(PowerState::D3Hot.to_string(), "D3Hot");
        assert_eq!(PowerState::D3Cold.to_string(), "D3Cold");
        assert_eq!(PowerState::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_power_state_default_is_unknown() {
        assert_eq!(PowerState::default(), PowerState::Unknown);
    }

    /*
        GpuVendor
    */

    #[test]
    fn test_gpu_vendor_from_amd_id() {
        assert_eq!(GpuVendor::from("0x1002"), GpuVendor::Amd);
    }

    #[test]
    fn test_gpu_vendor_from_nvidia_primary_id() {
        assert_eq!(GpuVendor::from("0x10de"), GpuVendor::Nvidia);
    }

    #[test]
    fn test_gpu_vendor_from_nvidia_alternate_ids() {
        assert_eq!(GpuVendor::from("0x104a"), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from("0x12d2"), GpuVendor::Nvidia);
    }

    #[test]
    fn test_gpu_vendor_from_intel_id() {
        assert_eq!(GpuVendor::from("0x8086"), GpuVendor::Intel);
    }

    #[test]
    fn test_gpu_vendor_from_unknown_id() {
        assert_eq!(GpuVendor::from("0x0000"), GpuVendor::Other);
        assert_eq!(GpuVendor::from("invalid"), GpuVendor::Other);
    }

    #[test]
    fn test_gpu_vendor_default_is_other() {
        assert_eq!(GpuVendor::default(), GpuVendor::Other);
    }

    /*
    GpuDevice
    */

    #[test]
    fn test_gpu_device_accessors_return_constructed_values() {
        let pci = make_pci();
        let gpu = GpuDevice::new(
            "RX 7900 XTX".to_string(),
            pci,
            128,
            0,
            Some(true),
            GpuVendor::Amd,
            None,
            true,
            false,
            true,
            false,
        );
        assert_eq!(gpu.name(), "RX 7900 XTX");
        assert_eq!(*gpu.render(), 128);
        assert_eq!(*gpu.card(), 0);
        assert_eq!(gpu.default(), Some(true));
        assert_eq!(gpu.gpu_vendor(), GpuVendor::Amd);
        assert_eq!(*gpu.nvidia_minor(), None);
        assert!(gpu.is_discrete());
        assert_eq!(gpu.pci().pci_address(), "0000:01:00.0");
    }

    #[test]
    fn test_gpu_device_is_default_true() {
        let gpu = GpuDevice::new(
            "GPU".to_string(),
            make_pci(),
            128,
            0,
            Some(true),
            GpuVendor::Amd,
            None,
            true,
            false,
            true,
            false,
        );
        assert!(gpu.is_default());
    }

    #[test]
    fn test_gpu_device_is_default_false() {
        let gpu = GpuDevice::new(
            "GPU".to_string(),
            make_pci(),
            128,
            0,
            Some(false),
            GpuVendor::Amd,
            None,
            true,
            false,
            true,
            false,
        );
        assert!(!gpu.is_default());
    }

    #[test]
    fn test_gpu_device_is_default_none_returns_false() {
        let gpu = GpuDevice::new(
            "GPU".to_string(),
            make_pci(),
            128,
            0,
            None,
            GpuVendor::Amd,
            None,
            false,
            false,
            true,
            false,
        );
        assert!(!gpu.is_default());
        assert!(!gpu.is_discrete());
    }

    #[test]
    fn test_gpu_device_set_default() {
        let mut gpu = GpuDevice::new(
            "GPU".to_string(),
            make_pci(),
            128,
            0,
            None,
            GpuVendor::Amd,
            None,
            true,
            false,
            true,
            false,
        );
        assert!(!gpu.is_default());
        gpu.set_default(Some(true));
        assert!(gpu.is_default());
    }

    #[test]
    fn test_gpu_device_with_nvidia_minor() {
        let gpu = GpuDevice::new(
            "RTX 4090".to_string(),
            make_pci(),
            128,
            0,
            Some(false),
            GpuVendor::Nvidia,
            Some(0),
            true,
            false,
            true,
            false,
        );
        assert_eq!(gpu.gpu_vendor(), GpuVendor::Nvidia);
        assert_eq!(*gpu.nvidia_minor(), Some(0));
        assert!(gpu.is_discrete());
    }
}
