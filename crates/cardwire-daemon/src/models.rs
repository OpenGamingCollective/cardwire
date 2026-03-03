use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::config::Config;
use cardwire_core::gpu;
use cardwire_core::iommu;
use cardwire_core::iommu::Device;
use cardwire_ebpf::EbpfBlocker;
use tokio::sync::{Mutex, RwLock};

#[derive(Deserialize, Serialize, PartialEq, zbus::zvariant::Type, Clone, Copy)]
pub enum Modes {
    Integrated,
    Hybrid,
    Manual,
}

impl fmt::Display for Modes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Modes::Integrated => write!(f, "Integrated"),
            Modes::Hybrid => write!(f, "Hybrid"),
            Modes::Manual => write!(f, "Manual"),
        }
    }
}

pub struct Daemon {
    pub state: DaemonState,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let pci_devices = iommu::read_pci_devices()?;
        let gpu_list = gpu::read_gpu(&pci_devices)?;
        let ebpf_blocker = cardwire_ebpf::EbpfBlocker::new()?;

        Ok(Self {
            state: DaemonState {
                config: tokio::sync::RwLock::new(config),
                _pci_devices: pci_devices,
                gpu_list,
                ebpf_blocker: tokio::sync::Mutex::new(ebpf_blocker),
            },
        })
    }
}

pub struct DaemonState {
    pub config: RwLock<Config>,
    pub gpu_list: HashMap<usize, gpu::Gpu>,
    // for future uses, related to vfio
    pub _pci_devices: HashMap<String, Device>,
    pub ebpf_blocker: Mutex<EbpfBlocker>,
}
impl DaemonState {
    pub async fn get_mode(&self) -> Modes {
        let config = self.config.read().await;
        config.mode
    }
}
