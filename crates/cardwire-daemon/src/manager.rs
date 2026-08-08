//! Daemon composition root: builds the shared [`DaemonContext`] and every D-Bus interface, owns
//! startup tasks and background-task futures.
use crate::{
    analyzer::CardwireAnalyzer, core::{
        env::compute_switcheroo_env, gpu::{GpuEnumerator, GpuVendor}, inode::exp_nvidia_inodes, pci::{self}
    }, file::{CardwireConfig, CardwireDatabase, CardwireGpuState, CardwireModeState}, interface::{
        ConfigInterface, ConfigMemory, DaemonContext, DebugInterface, GpuInterface, LoggerInterface, ModeInterface, Modes, SmartPolicyInterface, SwitcherooInterface
    }, tasks
};
use anyhow::{Context, Result};
use cardwire_ebpf_userspace::{EbpfBlocker, EbpfSettings};
use log::error;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use zbus::{fdo, interface};

#[derive(Clone)]
pub struct DaemonManager {
    pub mode_interface: ModeInterface,
    pub config_interface: ConfigInterface,
    pub debug_interface: DebugInterface,
    pub switcheroo_interface: SwitcherooInterface,
    pub logger_interface: LoggerInterface,
    pub smart_policy_interface: SmartPolicyInterface,
    pub inner: DaemonContext,
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

        let mut gpu_interfaces_map: BTreeMap<usize, Arc<GpuInterface>> = BTreeMap::new();
        let gpu_count = gpu_list
            .iter()
            .filter(|(_, gpu)| gpu.is_available())
            .count();
        for (id, device) in gpu_list {
            let gpu_env = compute_switcheroo_env(
                gpu_count,
                device.is_default(),
                device.is_discrete(),
                id as u32,
                device.gpu_vendor(),
                device.pci().pci_address(),
            );
            let gpu = GpuInterface::build(
                id as u32,
                device,
                gpu_env,
                Arc::clone(&blocker),
                Arc::clone(&pci_list),
                Arc::clone(&gpu_state),
                Arc::clone(&mode_state),
            )?;
            gpu_interfaces_map.insert(id, Arc::new(gpu));
        }
        let gpu_interfaces: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>> =
            Arc::new(RwLock::new(gpu_interfaces_map));

        let context = DaemonContext {
            mode_state,
            gpu_state,
            gpu_list: gpu_interfaces,
            config: user_config,
            blocker,
            power_tasks: Arc::new(RwLock::new(BTreeMap::new())),
            pci_list,
        };

        let mode_interface = ModeInterface::build(&context).await?;
        let logger_interface = LoggerInterface::build();
        let switcheroo_interface = SwitcherooInterface::build(Arc::clone(&context.gpu_list));

        Ok(Self {
            mode_interface: mode_interface.clone(),
            config_interface: ConfigInterface::build(&context)?,
            debug_interface: DebugInterface::build(
                &context,
                mode_interface.clone(),
                None,
                switcheroo_interface.clone(),
            )?,
            switcheroo_interface,
            logger_interface,
            smart_policy_interface,
            inner: context,
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
        let mode = Modes::try_from(mode_to_apply)?;
        // On first attempt: don't save (already persisted)
        // On fallback: persist so the broken mode isn't retried on every boot
        let save = mode_arg.is_some();
        self.mode_interface
            .internal_set_mode(mode, save)
            .await
            .map_err(anyhow::Error::from)
    }
    pub fn battery_switch_future(&self) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        let auto_switch = Arc::clone(&self.inner.config.battery_auto_switch);
        let auto_switch_mode = Arc::clone(&self.inner.config.battery_auto_switch_mode);
        let mode_interface = self.mode_interface.clone();
        async move {
            let res =
                tasks::watch_battery_status(auto_switch, auto_switch_mode, mode_interface).await;
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
    ) -> impl Future<Output = Result<(), zbus::Error>> + 'static {
        let mode = self.mode_interface.clone();
        let gpu_list = Arc::clone(&self.inner.gpu_list);
        async move {
            let res = tasks::monitor_display_changes(mode, gpu_list).await;
            if let Err(ref e) = res {
                error!("monitor_display task failed: {}", e);
            }
            res
        }
    }
    pub fn run_analyzer(&self) -> impl Future<Output = Result<(), anyhow::Error>> + 'static {
        let blocker = Arc::clone(&self.inner.blocker);
        let logger = Arc::clone(&self.logger_interface.report_logs);
        let signal = Arc::clone(&self.logger_interface.signal_emitter);
        let db_cache = self.smart_policy_interface.database.cache.clone();
        let tx = self.smart_policy_interface.database.tx.clone();

        let new_app_signal = Arc::clone(&self.smart_policy_interface.new_app_signal);

        async move {
            let cardwire_analyzer =
                CardwireAnalyzer::build(blocker, logger, signal, db_cache, tx, new_app_signal)
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
