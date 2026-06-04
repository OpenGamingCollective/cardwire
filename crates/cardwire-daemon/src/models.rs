//! where the struct and impl are declared
use crate::{
    core::{
        gpu::{self, check_default_drm_class}, pci
    }, file::{CardwireConfig, CardwireDatabase, CardwireGpuState, CardwireModeState}, interface::{
        ConfigInterface, ConfigMemory, DebugInterface, GpuInterface, ModeInterface, Modes
    }, profiler::CardwireProfiler
};
use anyhow::{Context, Result};
use cardwire_ebpf::{BlockKind, EbpfBlocker};
use log::warn;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use zbus::{
    fdo::{self}, interface
};

const BLOCKED_PCI_FILES: &[&str] = &[
    "config",
    "current_link_speed",
    "max_link_speed",
    "max_link_width",
    "current_link_width",
];
/// Files that get blocked when the NVIDIA block is on
const BLOCKED_NVIDIA_FILES: &[&str] = &[
    "libGLX_nvidia.so.0",
    "nvidia_icd.json",
    "nvidia_icd.x86_64.json",
    "nvidiactl",
];

#[derive(Clone)]
pub struct DaemonManager {
    pub mode_interface: ModeInterface,
    pub gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config_interface: ConfigInterface,
    pub debug_interface: DebugInterface,
    //pub cardwire_profiler: Arc<CardwireProfiler>
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

        let database = CardwireDatabase::build()?;
        let database = Arc::new(RwLock::new(database));

        let pci_devices: BTreeMap<String, pci::PciDevice> = pci::read_pci_devices()?;

        let mut gpu_list = gpu::read_gpu(&pci_devices)?;

        if let Err(err) = check_default_drm_class(&mut gpu_list) {
            warn!("Failed to determine default GPU: {}", err);
        }

        let pci_list: Arc<RwLock<BTreeMap<String, pci::PciDevice>>> =
            Arc::new(RwLock::new(pci_devices));

        let blocker = Arc::new(RwLock::new(EbpfBlocker::new()?));

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
            )?,
            gpu_interfaces: Arc::clone(&gpu_interfaces),
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
                Arc::clone(&database),
            )?,
            //cardwire_profiler: Arc::new(CardwireProfiler::build(ring_buffer, app_map, database,
            // close_map)?)
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
        let mut blocker = self.debug_interface.blocker.write().await;
        let mut state = self.debug_interface.gpu_state.write().await;
        blocker.block_kind(&config.to_string(), cardwire_ebpf::BlockKind::NvidiaSetting)?;

        for file in BLOCKED_PCI_FILES {
            blocker.block_kind(file, BlockKind::File)?;
        }
        let default: bool = state.is_default_state();
        // if there is an nvidia device, block nvidia file once
        for (_, gpu) in gpus_list.iter() {
            if gpu.device.nvidia() {
                for file in BLOCKED_NVIDIA_FILES {
                    blocker.block_kind(file, BlockKind::NvidiaFile)?;
                }
                break;
            }
        }
        if default {
            for (_, gpu) in gpus_list.iter() {
                state.save_state(&gpu.device, false).await?;
            }
        }
        // Dropping the locks prevent set_mode being stuck
        drop(blocker);
        drop(gpus_list);
        drop(state);
        let mode_to_apply = mode.mode();
        drop(mode);
        let mode_to_apply: u32 = match mode_to_apply {
            Modes::Integrated => 0,
            Modes::Hybrid => 1,
            Modes::Manual => 2,
            Modes::Smart => 3,
        };
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
    pub async fn refresh_gpu(&self) -> fdo::Result<()> {
        let _gpu_interfaces = self.gpu_interfaces.write().await;
        Ok(())
    }
}
