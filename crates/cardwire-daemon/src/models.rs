//! where the struct and impl are declared
use crate::{
    file::{CardwireConfig, CardwireGpuState, CardwireModeState}, gpu_dbus::Gpu
};
use anyhow::{Context, Result};
use cardwire_core::{
    gpu::{self, GpuBlocker, check_default_drm_class}, pci
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap}, fmt, sync::Arc
};
use tokio::sync::RwLock;
use zbus::fdo::Error;

const BLOCKED_PCI_FILES: &[&str] = &[
    "config",
    "current_link_speed",
    "max_link_speed",
    "max_link_width",
    "current_link_width",
];
// Files that get blocked when the NVIDIA block is on
const BLOCKED_NVIDIA_FILES: &[&str] = &[
    "libGLX_nvidia.so.0",
    "nvidia_icd.json",
    "nvidia_icd.x86_64.json",
    "nvidiactl",
];

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
    pub fn parse(input: &u32) -> zbus::fdo::Result<Modes> {
        match input {
            0 => Ok(Self::Integrated),
            1 => Ok(Self::Hybrid),
            2 => Ok(Self::Manual),
            unknown => Err(Error::InvalidArgs(format!(
                "unknown mode: {unknown} \n expected integrated|hybrid|manual"
            ))),
        }
    }
}

pub struct DaemonState {
    // these are file related
    pub gpu_state: RwLock<CardwireGpuState>,
    pub mode_state: RwLock<CardwireModeState>,
    // temp data
    pub gpu_list: BTreeMap<usize, Gpu>,
    pub ebpf_blocker: RwLock<GpuBlocker>,
    // for future uses, related to vfio
    pub pci_devices: BTreeMap<String, pci::PciDevice>,
}
pub struct ModeState {
    pub mode_config: Arc<RwLock<CardwireModeState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, Gpu>>>,
    pub config: Arc<RwLock<CardwireConfig>>,
}

pub struct ConfigState {
    pub config: Arc<RwLock<CardwireConfig>>,
}
pub struct GpuState {
    pub mode_config: Arc<RwLock<CardwireModeState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, Gpu>>>,
}
pub struct PciState {
    pub pci_list: Arc<RwLock<BTreeMap<String, pci::PciDevice>>>,
}
pub struct Daemon {
    pub mode_state: ModeState,
    pub gpu_state: GpuState,
    pub config_state: ConfigState,
    pub pci_state: PciState,
}

impl Daemon {
    pub async fn new() -> Result<Self> {
        //let mut gpu_state = CardwireGpuState::build().context("Error building gpu_state")?;
        let user_config = CardwireConfig::build().context("Error building toml config")?;
        let user_config: Arc<RwLock<CardwireConfig>> = Arc::new(RwLock::new(user_config));
        // Get mode from config
        let mode_config = CardwireModeState::build().context("Error building mode")?;
        let mode_config: Arc<RwLock<CardwireModeState>> = Arc::new(RwLock::new(mode_config));
        // TODO: Exit if no pci devices or manual refresh command
        let pci_devices: BTreeMap<String, pci::PciDevice> = pci::read_pci_devices()?;
        // TODO: Should the daemon exits if no gpu??
        let mut gpu_list = gpu::read_gpu(&pci_devices)?;
        // Executed after the read_gpu to use the current gpu_list
        if let Err(err) = check_default_drm_class(&mut gpu_list) {
            warn!("Failed to determine default GPU: {}", err);
        }
        let pci_list: Arc<RwLock<BTreeMap<String, pci::PciDevice>>> =
            Arc::new(RwLock::new(pci_devices));
        // Exit if ebpf returns an error
        let blocker = Arc::new(RwLock::new(GpuBlocker::new()?));
        // create gpu list
        let mut new_gpu_list: BTreeMap<usize, Gpu> = BTreeMap::new();
        for (id, gpu) in gpu_list {
            new_gpu_list.insert(
                id,
                Gpu::new(Arc::clone(&blocker), gpu, Arc::clone(&pci_list)),
            );
        }
        let gpu_list: Arc<RwLock<BTreeMap<usize, Gpu>>> = Arc::new(RwLock::new(new_gpu_list));

        // Create GpuState
        let gpu_state: GpuState = GpuState {
            mode_config: Arc::clone(&mode_config),
            gpu_list: Arc::clone(&gpu_list),
        };

        let mode_state: ModeState = ModeState {
            mode_config: Arc::clone(&mode_config),
            gpu_list: Arc::clone(&gpu_list),
            config: Arc::clone(&user_config),
        };
        let config_state: ConfigState = ConfigState {
            config: Arc::clone(&user_config),
        };
        let pci_state: PciState = PciState {
            pci_list: Arc::clone(&pci_list),
        };

        Ok(Self {
            mode_state,
            gpu_state,
            config_state,
            pci_state,
        })
    }
}
