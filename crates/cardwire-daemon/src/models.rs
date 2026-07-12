//! where the struct and impl are declared
use crate::{
    analyzer::CardwireAnalyzer, core::{
        gpu::{self, GpuVendor, check_default_drm_class}, inode::exp_nvidia_inodes, pci
    }, file::{CardwireConfig, CardwireGpuState, CardwireModeState}, interface::{
        ConfigInterface, ConfigMemory, DebugInterface, GpuInterface, ModeInterface, Modes
    }
};
use anyhow::{Context, Result};
use cardwire_ebpf::{EbpfBlocker, EbpfSettings};
use log::{error, warn};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use zbus::{
    fdo::{self}, interface
};

// shouldn't be necessary anymore
const ALLOWED_PROGRAMS: &[&str] = &["(udev-worker)", "pacman", "nix", "dnf", "apt"];

#[derive(Clone)]
pub struct DaemonManager {
    pub mode_interface: ModeInterface,
    pub gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config_interface: ConfigInterface,
    pub debug_interface: DebugInterface,
    pub cardwire_analyzer: CardwireAnalyzer,
    pub power_tasks: Arc<RwLock<BTreeMap<usize, tokio::task::JoinHandle<anyhow::Result<()>>>>>,
}

impl DaemonManager {
    pub async fn new() -> Result<Self> {
        let mode_state: CardwireModeState =
            CardwireModeState::build().context("Error building mode")?;
        let mode_state: Arc<RwLock<CardwireModeState>> = Arc::new(RwLock::new(mode_state));

        let user_config: CardwireConfig =
            CardwireConfig::build().context("Error building toml config")?;
        let user_config = Arc::new(ConfigMemory::build(user_config));

        let gpu_state: CardwireGpuState = CardwireGpuState::build()?;
        let gpu_state: Arc<RwLock<CardwireGpuState>> = Arc::new(RwLock::new(gpu_state));

        let pci_devices: BTreeMap<String, pci::PciDevice> = pci::read_pci_devices()?;

        let mut gpu_list = gpu::read_gpu(&pci_devices)?;

        if let Err(err) = check_default_drm_class(&mut gpu_list) {
            warn!("Failed to determine default GPU: {}", err);
        }

        let pci_list: Arc<RwLock<BTreeMap<String, pci::PciDevice>>> =
            Arc::new(RwLock::new(pci_devices));

        let blocker = Arc::new(RwLock::new(EbpfBlocker::new()?));

        let power_tasks = Arc::new(RwLock::new(BTreeMap::new()));

        let mut gpu_interfaces_map: BTreeMap<usize, GpuInterface> = BTreeMap::new();

        for (id, device) in gpu_list {
            let gpu = GpuInterface::build(
                device,
                Arc::clone(&blocker),
                Arc::clone(&pci_list),
                Arc::clone(&gpu_state),
                Arc::clone(&mode_state),
            )?;
            gpu_interfaces_map.insert(id, gpu);
        }

        let gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>> =
            Arc::new(RwLock::new(gpu_interfaces_map));

        Ok(Self {
            mode_interface: ModeInterface::build(
                Arc::clone(&mode_state),
                Arc::clone(&gpu_state),
                Arc::clone(&gpu_interfaces),
                Arc::clone(&user_config),
                Arc::clone(&blocker),
            )
            .await?,
            gpu_interfaces: Arc::clone(&gpu_interfaces),
            power_tasks: Arc::clone(&power_tasks),
            config_interface: ConfigInterface::build(
                Arc::clone(&user_config),
                Arc::clone(&blocker),
            )?,
            debug_interface: DebugInterface::build(
                Arc::clone(&mode_state),
                Arc::clone(&gpu_state),
                Arc::clone(&gpu_interfaces),
                Arc::clone(&user_config),
                Arc::clone(&blocker),
                Arc::clone(&pci_list),
                None,
                Arc::clone(&power_tasks),
            )?,
            cardwire_analyzer: CardwireAnalyzer::build(Arc::clone(&blocker)).await?,
        })
    }

    /// Tasks that need to be run before running the daemon, like applying the mode,
    pub async fn pre_daemon_tasks(&self) -> Result<()> {
        let gpus_list = self.debug_interface.gpu_list.read().await;
        let config = self
            .debug_interface
            .config
            .experimental_nvidia_block
            .load(std::sync::atomic::Ordering::Relaxed);
        let mode = self.debug_interface.mode_state.read().await;
        let mut state = self.debug_interface.gpu_state.write().await;
        let mut blocker = self.debug_interface.blocker.write().await;
        // Whitelist cardwire pid before starting
        let pid = std::process::id();
        if let Err(err) = blocker.whitelist_cardwire_pid(pid) {
            error!("failed to whitelist cardwire's pid: {}", err);
            return Err(err.into());
        };

        // Set nvidia setting
        blocker.set_ebpf_setting(EbpfSettings::ExperimentalNvidia, config.into())?;
        // Push nvidia inodes, if empty/error just ignore
        for (_, gpu) in gpus_list.iter() {
            if gpu.device.gpu_vendor() == GpuVendor::Nvidia
                && let Ok(inodes) = exp_nvidia_inodes()
                && !inodes.is_empty()
            {
                for inode in inodes {
                    if let Err(err) = blocker.block_exp_inode(inode) {
                        error!("failed to block nvidia's file {}: {}", inode, err);
                    }
                }
                break;
            }
        }

        for comm in ALLOWED_PROGRAMS {
            blocker.allow_comm(comm)?;
        }

        drop(blocker);

        let default: bool = state.is_default_state();
        if default {
            for (_, gpu) in gpus_list.iter() {
                state.save_state(&gpu.device, false).await?;
            }
        }
        // Dropping the locks prevent set_mode being stuck
        drop(gpus_list);
        drop(state);
        let mode_to_apply = Modes::into(mode.mode());
        drop(mode);
        self.mode_interface.set_mode(mode_to_apply).await?;
        Ok(())
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Manager")]
// simple dbus to check if the daemon is alive
impl DaemonManager {
    pub async fn status(&self) -> fdo::Result<()> {
        Ok(())
    }
}
