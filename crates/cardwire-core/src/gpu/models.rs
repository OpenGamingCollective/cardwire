use crate::pci::PciDevice;

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct GpuDevice {
    name: String,
    pub pci: PciDevice,
    render: u32,
    card: u32,
    default: Option<bool>,
    nvidia: bool,
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
    pub fn nvidia(&self) -> bool {
        self.nvidia
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
        nvidia: bool,
        nvidia_minor: Option<u32>,
    ) -> GpuDevice {
        GpuDevice {
            name,
            pci,
            render,
            card,
            default,
            nvidia,
            nvidia_minor,
        }
    }

    pub fn is_default(&self) -> bool {
        self.default.unwrap_or(false)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
pub struct DbusGpuDevice {
    pub id: u32,
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub blocked: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}
