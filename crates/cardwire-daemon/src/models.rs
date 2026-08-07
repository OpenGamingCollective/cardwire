//! where the struct and impl are declared
use crate::{
    analyzer::CardwireAnalyzer, core::{
        gpu::{GpuEnumerator, GpuVendor}, inode::exp_nvidia_inodes, pci::{self}
    }, file::{CardwireConfig, CardwireDatabase, CardwireGpuState, CardwireModeState}, interface::{
        ConfigInterface, ConfigMemory, DebugInterface, GpuInterface, LoggerInterface, ModeInterface, Modes, SmartPolicyInterface, SwitcherooInterface
    }, tasks
};
use anyhow::{Context, Result};
use cardwire_ebpf_userspace::{EbpfBlocker, EbpfSettings};
use log::error;
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::RwLock, task};
use zbus::{
    fdo::{self}, interface, object_server::{InterfaceRef, SignalEmitter}
};

/// Contain the variable used by the daemon in daemon.rs
#[derive(Clone)]
pub struct DaemonInner {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config: Arc<ConfigMemory>,
    pub blocker: Arc<RwLock<EbpfBlocker>>,
    pub power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<anyhow::Result<()>>>>>,
}

#[derive(Clone)]
pub struct DaemonManager {
    pub mode_interface: ModeInterface,
    pub gpu_interfaces: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config_interface: ConfigInterface,
    pub debug_interface: DebugInterface,
    pub switcheroo_interface: SwitcherooInterface,
    pub logger_interface: LoggerInterface,
    pub logger_signal: Option<SignalEmitter<'static>>,
    pub smart_policy_interface: SmartPolicyInterface,
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

        let gpu_enumerator = GpuEnumerator::build();
        let gpu_list = gpu_enumerator.enumerate(&pci_devices);

        let pci_list: Arc<RwLock<BTreeMap<String, pci::PciDevice>>> =
            Arc::new(RwLock::new(pci_devices));

        let mut blocker = EbpfBlocker::new()?;

        let database = CardwireDatabase::build()?;

        let smart_policy_interface = SmartPolicyInterface::build(&mut blocker, database);

        let blocker = Arc::new(RwLock::new(blocker));

        let power_tasks = Arc::new(RwLock::new(BTreeMap::new()));

        let mut gpu_interfaces_map: BTreeMap<usize, GpuInterface> = BTreeMap::new();

        for (id, device) in gpu_list {
            let gpu = GpuInterface::build(
                id as u32,
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

        let mode_interface = ModeInterface::build(
            Arc::clone(&mode_state),
            Arc::clone(&gpu_state),
            Arc::clone(&gpu_interfaces),
            Arc::clone(&user_config),
            Arc::clone(&blocker),
        )
        .await?;

        let logger_interface = LoggerInterface::build();

        let switcheroo_interface = SwitcherooInterface::build(Arc::clone(&gpu_interfaces));

        Ok(Self {
            mode_interface: mode_interface.clone(),
            gpu_interfaces: Arc::clone(&gpu_interfaces),
            config_interface: ConfigInterface::build(
                Arc::clone(&user_config),
                Arc::clone(&blocker),
            )?,
            debug_interface: DebugInterface::build(
                Arc::clone(&mode_state),
                mode_interface.clone(),
                Arc::clone(&gpu_state),
                Arc::clone(&gpu_interfaces),
                Arc::clone(&user_config),
                Arc::clone(&blocker),
                Arc::clone(&pci_list),
                None,
                Arc::clone(&power_tasks),
                switcheroo_interface.clone(),
            )?,
            switcheroo_interface,
            logger_interface,
            logger_signal: None,
            smart_policy_interface,
            inner: DaemonInner {
                mode_state: Arc::clone(&mode_state),
                gpu_state: Arc::clone(&gpu_state),
                gpu_list: Arc::clone(&gpu_interfaces),
                config: Arc::clone(&user_config),
                blocker: Arc::clone(&blocker),
                power_tasks: Arc::clone(&power_tasks),
            },
        })
    }

    /// Tasks that need to be run before running the daemon, like applying the mode,
    pub async fn pre_daemon_tasks(&self) -> Result<()> {
        // Whitelist cardwire pid before starting
        self.whitelist_daemon_pid().await?;

        // Set nvidia setting
        self.set_nvidia_setting().await?;
        // Push nvidia inodes, if empty/error just ignore
        self.block_nvidia_inodes().await?;

        // Add some programs to the whitelisted comm map
        self.whitelist_programs().await?;

        // If it's the first time cardwired is launched, we need to populate the gpu state file
        self.populate_state_file().await?;

        // This one can fail on asus laptop when switching to integrated using the kernel attribute
        if let Err(err) = self.apply_mode_at_startup(None).await {
            error!(
                "failed to apply mode at startup: {}, switching to manual...",
                err
            );
            // 2 = manual
            self.apply_mode_at_startup(Some(2)).await?
        };

        Ok(())
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
        for (id, gpu) in gpus_list.iter() {
            if gpu.device.gpu_vendor() == GpuVendor::Nvidia
                && !gpu.device.is_default()
                && let Ok(inodes) = exp_nvidia_inodes()
                && !inodes.is_empty()
            {
                for inode in inodes {
                    if let Err(err) = blocker.block_exp_inode(inode, *id as u32) {
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
        const ALLOWED_PROGRAMS: &[&str] = &[
            "(udev-worker)",
            "systemd-udevd",
            "pacman",
            "dnf",
            "apt",
            "nix",
            "nix-daemon",
        ];

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
            for gpu in gpus_list.values() {
                state.save_state(&gpu.device, false).await?;
            }
        }
        Ok(())
    }
    async fn apply_mode_at_startup(&self, mode_arg: Option<u32>) -> Result<()> {
        // If a mode is supplied as arg, use it, else read the internal state (from file)
        let mode_to_apply = match mode_arg {
            Some(mode) => mode,
            None => {
                let mode_lock = self.inner.mode_state.read().await;
                Modes::into(mode_lock.mode())
            }
        };
        let mode = Modes::try_from(mode_to_apply).map_err(anyhow::Error::msg)?;
        // Store the result to return after any fallback mode state has been updated.
        let res = self
            .mode_interface
            .apply_mode_at_startup(mode, true)
            .await
            .map_err(anyhow::Error::from);
        // If the configured mode failed, persist the supplied fallback instead of retrying the
        // failing mode on every daemon start.
        if mode_arg.is_some() {
            let mut mode_lock = self.inner.mode_state.write().await;
            mode_lock.save_state(mode).await?;
        }
        res
    }
    pub fn battery_switch_future(&self) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        let auto_switch = Arc::clone(&self.inner.config.battery_auto_switch);
        let auto_switch_mode = Arc::clone(&self.inner.config.battery_auto_switch_mode);
        async move {
            let res = tasks::watch_battery_status(auto_switch, auto_switch_mode).await;
            if let Err(ref e) = res {
                error!("battery_switch task failed: {}", e);
            }
            res
        }
    }
    pub fn monitor_udev_future(&self) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        let debug_int = self.debug_interface.clone();
        async move {
            let res = tasks::monitor_pci_changes(debug_int).await;
            if let Err(ref e) = res {
                error!("monitor_udev task failed: {}", e);
            }
            res
        }
    }
    pub fn monitor_display_future(
        &self,
        mode_interface: InterfaceRef<ModeInterface>,
    ) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        // Clone the shared D-Bus interface into the long-running monitor task.
        let mode = self.mode_interface.clone();
        async move {
            let res = tasks::monitor_display_changes(mode, mode_interface).await;
            if let Err(ref e) = res {
                error!("monitor_display task failed: {}", e);
            }
            res
        }
    }
    pub fn run_analyzer(&self) -> impl Future<Output = Result<(), anyhow::Error>> + 'static {
        let blocker = Arc::clone(&self.inner.blocker);
        let logger = Arc::clone(&self.logger_interface.report_logs);
        let signal = self.logger_signal.clone();
        let db_cache = self.smart_policy_interface.database.cache.clone();
        let tx = self.smart_policy_interface.database.tx.clone();

        async move {
            let cardwire_analyzer = CardwireAnalyzer::build(blocker, logger, signal, db_cache, tx)
                .await
                .map_err(|err| {
                    error!("Failed to build CardwireAnalyzer: {}", err);
                    err
                })?;
            let res = cardwire_analyzer.run().await;
            if let Err(ref e) = res {
                error!("CardwireAnalyzer task failed: {}", e);
            }
            res
        }
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Manager")]
// simple dbus to check if the daemon is alive
impl DaemonManager {
    pub async fn status(&self) -> fdo::Result<()> {
        Ok(())
    }
}
