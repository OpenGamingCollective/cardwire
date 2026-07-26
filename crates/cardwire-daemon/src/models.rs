//! where the struct and impl are declared
use crate::{
    analyzer::CardwireAnalyzer, core::{
        gpu::{self, GpuVendor, check_default_drm_class}, inode::exp_nvidia_inodes, pci::{self}
    }, file::{CardwireConfig, CardwireGpuState, CardwireModeState}, interface::{
        ConfigInterface, ConfigMemory, DebugInterface, GpuInterface, ModeInterface, Modes
    }, tasks
};
use anyhow::{Context, Result};
use cardwire_ebpf::{EbpfBlocker, EbpfSettings};
use log::error;
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::RwLock, task};
use zbus::{
    fdo::{self}, interface
};

/// Contain the variable used by the daemon in daemon.rs
#[derive(Clone)]
pub struct DaemonInner {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config: Arc<ConfigMemory>,
    pub blocker: Arc<RwLock<EbpfBlocker>>,
    #[allow(dead_code)]
    pub object_server: Arc<RwLock<Option<zbus::ObjectServer>>>,
    pub power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<anyhow::Result<()>>>>>,
}

#[derive(Clone)]
pub struct DaemonManager {
    pub mode_interface: ModeInterface,
    pub gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config_interface: ConfigInterface,
    pub debug_interface: DebugInterface,
    pub inner: DaemonInner,
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

        let gpu_list = gpu::read_gpu(&pci_devices)?;

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

        let object_serv: Arc<RwLock<Option<zbus::ObjectServer>>> = Arc::new(RwLock::default());

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
            inner: DaemonInner {
                mode_state: Arc::clone(&mode_state),
                gpu_state: Arc::clone(&gpu_state),
                gpu_list: Arc::clone(&gpu_interfaces),
                config: Arc::clone(&user_config),
                blocker: Arc::clone(&blocker),
                object_server: Arc::clone(&object_serv),
                power_tasks: Arc::clone(&power_tasks),
            },
        })
    }

    /// Tasks that need to be run before running the daemon, like applying the mode,
    pub async fn pre_daemon_tasks(&self) -> Result<()> {
        // Whitelist cardwire pid before starting
        self.whitelist_daemon_pid().await?;

        // Find the default gpu
        self.populate_default_gpu().await?;

        // Set nvidia setting
        self.set_nvidia_setting().await?;
        // Push nvidia inodes, if empty/error just ignore
        self.block_nvidia_inodes().await?;

        // Add some programs to the whitelisted comm map
        self.whitelist_programs().await?;

        // If it's the first time cardwired is launched, we need to populate the gpu state file
        self.populate_state_file().await?;

        self.apply_mode_at_startup().await?;

        Ok(())
    }

    async fn populate_default_gpu(&self) -> Result<()> {
        let mut gpu_interface = self.inner.gpu_list.write().await;
        check_default_drm_class(&mut gpu_interface).map_err(|err| err.into())
    }

    /// Whitelist the daemon pid inside the ebpf program
    async fn whitelist_daemon_pid(&self) -> Result<()> {
        // Get lock on ebpf-blocker
        let mut blocker = self.inner.blocker.write().await;
        // Get the process pid
        let pid = std::process::id();
        // Now insert the process's pid into the ebpf map
        blocker
            .whitelist_cardwire_pid(pid)
            .map_err(|err| err.into())
    }
    /// Set the ebpf nvidia setting state
    async fn set_nvidia_setting(&self) -> Result<()> {
        // Get lock on ebpf-blocker
        let mut blocker = self.inner.blocker.write().await;
        blocker
            .set_ebpf_setting(
                EbpfSettings::ExperimentalNvidia,
                self.debug_interface
                    .config
                    .experimental_nvidia_block
                    .load(std::sync::atomic::Ordering::Relaxed)
                    .into(),
            )
            .map_err(|err| err.into())
    }
    async fn block_nvidia_inodes(&self) -> Result<()> {
        let gpus_list = self.inner.gpu_list.read().await;
        let mut blocker = self.inner.blocker.write().await;
        // Only block if the device has a Nvidia gpu
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
        Ok(())
    }
    async fn whitelist_programs(&self) -> Result<()> {
        // List of allowed programs
        const ALLOWED_PROGRAMS: &[&str] =
            &["(udev-worker)", "pacman", "dnf", "apt", "nix", "nix-daemon"];

        let mut blocker = self.inner.blocker.write().await;

        // Iter over the ALLOWED_PROGRAMS array and allow each comm
        for comm in ALLOWED_PROGRAMS {
            blocker.allow_comm(comm)?;
        }
        Ok(())
    }
    async fn populate_state_file(&self) -> Result<()> {
        let gpus_list = self.inner.gpu_list.read().await;
        let mut state = self.inner.gpu_state.write().await;
        let default: bool = state.is_default_state();
        if default {
            for (_, gpu) in gpus_list.iter() {
                state.save_state(&gpu.device, false).await?;
            }
        }
        Ok(())
    }
    async fn apply_mode_at_startup(&self) -> Result<()> {
        let mode_to_apply = {
            let mode = self.inner.mode_state.read().await;
            Modes::into(mode.mode())
        };
        self.mode_interface
            .set_mode(mode_to_apply)
            .await
            .map_err(|err| err.into())
    }
    pub fn battery_switch_future(&self) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        tasks::watch_battery_status(
            Arc::clone(&self.inner.config.battery_auto_switch),
            Arc::clone(&self.inner.config.battery_auto_switch_mode),
        )
    }
    pub fn monitor_udev_future(&self) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        tasks::monitor_pci_changes(self.debug_interface.clone())
    }
    pub fn run_analyzer(&self) -> impl Future<Output = Result<(), anyhow::Error>> + 'static {
        let blocker = Arc::clone(&self.inner.blocker);
        async move {
            let cardwire_analyzer = CardwireAnalyzer::build(Arc::clone(&blocker)).await.unwrap();
            cardwire_analyzer.run().await
        }
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Manager")]
// simple dbus to check if the daemon is alive
impl DaemonManager {
    pub async fn status(&self) -> fdo::Result<()> {
        Ok(())
    }
}
