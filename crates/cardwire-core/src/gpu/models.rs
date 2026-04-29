#[derive(Clone)]
pub struct Gpu {
    pub id: u32,
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: Option<bool>,
    pub nvidia: bool,
    pub nvidia_minor: Option<u32>,
}
impl Gpu {
    pub fn pci_address(&self) -> &str {
        &self.pci
    }

    pub fn is_default(&self) -> bool {
        self.default.unwrap_or_default()
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn render_node(&self) -> &u32 {
        &self.render
    }

    pub fn card_node(&self) -> &u32 {
        &self.card
    }
    pub fn is_nvidia(&self) -> &bool {
        &self.nvidia
    }
    pub fn nvidia_minor(&self) -> &Option<u32> {
        &self.nvidia_minor
    }
}

// GpuRow for display
pub type GpuRow = (u32, String, String, String, bool, bool);
