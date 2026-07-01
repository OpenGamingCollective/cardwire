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
            "D3Hot" => Ok(PowerState::D3Hot),
            "D3Cold" => Ok(PowerState::D3Cold),
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

    pub fn new(
        name: String,
        pci: PciDevice,
        render: u32,
        card: u32,
        default: Option<bool>,
        gpu_vendor: GpuVendor,
        nvidia_minor: Option<u32>,
    ) -> GpuDevice {
        GpuDevice {
            name,
            pci,
            render,
            card,
            default,
            gpu_vendor,
            nvidia_minor,
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
