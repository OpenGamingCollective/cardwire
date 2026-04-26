use crate::config::{CardwireConfig, CardwireGpuState, CardwireModeState};
use anyhow::{Context, Result};
use cardwire_core::{
    gpu::{self, GpuBlocker, check_default_drm_class}, pci
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

pub struct DaemonState {
    pub config: RwLock<CardwireConfig>,
    pub gpu_state: RwLock<CardwireGpuState>,
    pub mode_state: RwLock<CardwireModeState>,
    pub gpu_list: HashMap<usize, gpu::Gpu>,
    pub ebpf_blocker: RwLock<GpuBlocker>,
    // for future uses, related to vfio
    pub _pci_devices: HashMap<String, pci::PciDevice>,
    pub _iommu: bool,
}
impl DaemonState {
    pub async fn _iommu(&self) -> bool {
        self._iommu
    }
}

pub struct Daemon {
    pub state: DaemonState,
}

impl Daemon {
    pub async fn new() -> Result<Self> {
        let iommu: bool = pci::is_iommu_enabled();
        let config = CardwireConfig::build().context("Error building config")?;
        let mut gpu_state = CardwireGpuState::build().context("Error building gpu_state")?;
        let mode_state = CardwireModeState::build().context("Error building mode")?;
        // TODO: Exit if no pci devices or manual refresh command
        let pci_devices = pci::read_pci_devices()?;
        // TODO: Should the daemon exits if no gpu??
        let mut gpu_list = gpu::read_gpu(&pci_devices)?;
        // Executed after the read_gpu to use the current gpu_list
        if let Err(err) = check_default_drm_class(&mut gpu_list) {
            warn!("Failed to determine default GPU: {}", err);
        }
        // TODO: Exit if ebpf returns an error, or try to recover from it?
        let ebpf_blocker = GpuBlocker::new()?;
        // Do not stop the program if there is no gpu, cardwire will also be usable as a pci manager
        // in a near future
        if !gpu_list.is_empty() && gpu_state.is_default_state() {
            gpu_state
                .save_state(&gpu_list, &ebpf_blocker)
                .await
                .context("Could not save gpu state")?;
        } else {
            warn!("could not detect gpus, daemon is still running for pci management usage")
        }

        Ok(Self {
            state: DaemonState {
                config: RwLock::new(config),
                gpu_state: RwLock::new(gpu_state),
                mode_state: RwLock::new(mode_state),
                _pci_devices: pci_devices,
                _iommu: iommu,
                gpu_list,
                ebpf_blocker: RwLock::new(ebpf_blocker),
            },
        })
    }
    pub async fn apply_config(&mut self) -> anyhow::Result<()> {
        let config = self.state.config.read().await;
        let mode = self.state.mode_state.read().await;
        let mut blocker = self.state.ebpf_blocker.write().await;
        // Apply vulkan block
        blocker.set_vulkan_block(config.block_nvidia_vulkan())?;
        // Dropping the locks prevent set_mode being stuck
        drop(blocker);
        drop(config);
        // Apply mode
        let mode_to_apply = mode.mode().to_string();
        drop(mode);
        self.set_mode(mode_to_apply).await?;

        Ok(())
    }
}
