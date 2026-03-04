use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::config::Config;
use cardwire_core::gpu::{self, block_gpu};
use cardwire_core::iommu;
use cardwire_core::iommu::Device;
use cardwire_ebpf::EbpfBlocker;
use log::warn;
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
        let mut ebpf_blocker = cardwire_ebpf::EbpfBlocker::new()?;

        // Apply config mode at startup
        match config.mode {
            Modes::Integrated | Modes::Hybrid => {
                let block = config.mode == Modes::Integrated;
                for gpu in gpu_list.values() {
                    if !gpu.is_default() {
                        if let Err(err) = block_gpu(&mut ebpf_blocker, gpu, block) {
                            warn!("Failed to apply config mode {} at startup: {}", config.mode, err);
                        }
                    }
                }
            }
            Modes::Manual => {}
        }

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
