use crate::config::Config;
use anyhow::Result;
use cardwire_core::{
    gpu::{self, GpuBlocker, block_gpu}, pci
};
use log::warn;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use tokio::sync::RwLock;
use zbus::fdo::Error;

#[derive(Deserialize, Serialize, PartialEq, zbus::zvariant::Type, Clone, Copy, Default)]
pub enum Modes {
    Integrated,
    Hybrid,
    #[default]
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

impl Modes {
    pub fn parse(input: &str) -> zbus::fdo::Result<Modes> {
        match input.to_ascii_lowercase().as_str() {
            "integrated" => Ok(Self::Integrated),
            "hybrid" => Ok(Self::Hybrid),
            "manual" => Ok(Self::Manual),
            unknown => Err(Error::InvalidArgs(format!(
                "unknown mode: {unknown} \n expected integrated|hybrid|manual"
            ))),
        }
    }
}

pub struct Daemon {
    pub state: DaemonState,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Self> {
        // TODO: what if no iommu folder
        let iommu: bool = pci::is_iommu_enabled();
        // TODO: what if no pci device
        let pci_devices = pci::read_pci_devices()?;
        // TODO: what if couldn't find gpu
        let gpu_list = gpu::read_gpu(&pci_devices)?;
        // TODO: what if ebpf crash
        let mut ebpf_blocker = GpuBlocker::new()?;

        // Apply config block_vulkan at startup
        ebpf_blocker.set_vulkan_block(config.block_vulkan)?;

        // Apply config mode at startup
        // TODO: use already existing function set_mode()
        match config.mode {
            Modes::Integrated | Modes::Hybrid => {
                let block = config.mode == Modes::Integrated;
                for gpu in gpu_list.values() {
                    if !gpu.is_default()
                        && let Err(err) = block_gpu(&mut ebpf_blocker, gpu, block)
                    {
                        warn!(
                            "Failed to apply config mode {} at startup: {}",
                            config.mode, err
                        );
                    }
                }
            }
            Modes::Manual => {}
        }

        Ok(Self {
            state: DaemonState {
                config: tokio::sync::RwLock::new(config),
                _pci_devices: pci_devices,
                _iommu: iommu,
                gpu_list,
                ebpf_blocker: tokio::sync::RwLock::new(ebpf_blocker),
            },
        })
    }
}
pub struct DaemonState {
    pub config: RwLock<Config>,
    pub gpu_list: HashMap<usize, gpu::Gpu>,
    pub ebpf_blocker: RwLock<GpuBlocker>,
    // for future uses, related to vfio
    pub _pci_devices: HashMap<String, pci::PciDevice>,
    pub _iommu: bool,
}
impl DaemonState {
    pub async fn mode(&self) -> Modes {
        let config = self.config.read().await;
        config.mode
    }
    pub async fn _is_iommu(&self) -> bool {
        self._iommu
    }
}
