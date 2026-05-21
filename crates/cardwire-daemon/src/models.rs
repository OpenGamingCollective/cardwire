//! where the struct and impl are declared
use crate::{
    file::{CardwireConfig, CardwireGpuState, CardwireModeState}, interface::{GpuInterface, ModeInterface}
};
use anyhow::{Context, Result};
use cardwire_core::{
    gpu::{self, GpuBlocker, check_default_drm_class}, pci
};
use log::warn;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use zbus::{
    fdo::{self}, interface
};

//const BLOCKED_PCI_FILES: &[&str] = &[
//    "config",
//    "current_link_speed",
//    "max_link_speed",
//    "max_link_width",
//    "current_link_width",
//];
/// Files that get blocked when the NVIDIA block is on
//const BLOCKED_NVIDIA_FILES: &[&str] = &[
//    "libGLX_nvidia.so.0",
//    "nvidia_icd.json",
//    "nvidia_icd.x86_64.json",
//    "nvidiactl",
//];

#[derive(Clone)]
pub struct DaemonManager {
    pub mode_interface: ModeInterface,
    pub gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
}

impl DaemonManager {
    pub async fn new() -> Result<Self> {
        let mode_state: CardwireModeState =
            CardwireModeState::build().context("Error building mode")?;
        let mode_state: Arc<RwLock<CardwireModeState>> = Arc::new(RwLock::new(mode_state));

        let user_config = CardwireConfig::build().context("Error building toml config")?;
        let user_config: Arc<RwLock<CardwireConfig>> = Arc::new(RwLock::new(user_config));

        let gpu_state: CardwireGpuState = CardwireGpuState::build()?;
        let gpu_state: Arc<RwLock<CardwireGpuState>> = Arc::new(RwLock::new(gpu_state));

        let pci_devices: BTreeMap<String, pci::PciDevice> = pci::read_pci_devices()?;

        let mut gpu_list = gpu::read_gpu(&pci_devices)?;

        if let Err(err) = check_default_drm_class(&mut gpu_list) {
            warn!("Failed to determine default GPU: {}", err);
        }

        let pci_list: Arc<RwLock<BTreeMap<String, pci::PciDevice>>> =
            Arc::new(RwLock::new(pci_devices));

        let blocker = Arc::new(RwLock::new(GpuBlocker::new()?));

        let mut gpu_interfaces_map: BTreeMap<usize, GpuInterface> = BTreeMap::new();

        for (id, device) in gpu_list {
            let gpu = GpuInterface::build(
                device,
                Arc::clone(&blocker),
                Arc::clone(&pci_list),
                Arc::clone(&gpu_state),
            )?;
            gpu_interfaces_map.insert(id, gpu);
        }

        let gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>> =
            Arc::new(RwLock::new(gpu_interfaces_map));

        Ok(Self {
            mode_interface: ModeInterface::build(
                mode_state,
                Arc::clone(&gpu_state),
                Arc::clone(&gpu_interfaces),
                Arc::clone(&user_config),
            )?,
            gpu_interfaces: Arc::clone(&gpu_interfaces),
        })
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Manager")]
// simple dbus to check if the daemon is alive
impl DaemonManager {
    pub async fn status(&self) -> fdo::Result<()> {
        Ok(())
    }
}
