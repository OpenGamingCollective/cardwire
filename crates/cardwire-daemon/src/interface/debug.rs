use crate::{
    core::{
        env::compute_switcheroo_env, gpu::{GpuEnumerator, GpuVendor}, inode::exp_nvidia_inodes, pci::{self, DbusPciDevice, PciDevice}
    }, interface::SwitcherooInterface, tasks::watch_power_state
};
use cardwire_ebpf_userspace::{EbpfBlocker, InodeKey};
use log::{error, info, warn};
use std::{
    collections::{BTreeMap, HashSet}, sync::Arc
};
use tokio::{sync::RwLock, task};
use zbus::{fdo, interface};

use crate::{
    Result, file::{CardwireGpuState, CardwireModeState}, interface::{ConfigMemory, DaemonContext, GpuInterface, ModeInterface, Modes}
};

#[derive(Clone)]
pub struct DebugInterface {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub mode_interface: ModeInterface,
    pub gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
    pub config: Arc<ConfigMemory>,
    pub blocker: Arc<RwLock<EbpfBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    pub object_server: Option<zbus::ObjectServer>,
    pub power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<Result<()>>>>>,
    pub switcheroo: SwitcherooInterface,
}
impl DebugInterface {
    pub fn build(
        context: &DaemonContext,
        mode_interface: ModeInterface,
        object_server: Option<zbus::ObjectServer>,
        switcheroo: SwitcherooInterface,
    ) -> Result<DebugInterface> {
        Ok(DebugInterface {
            mode_state: context.mode_state.clone(),
            mode_interface,
            gpu_state: context.gpu_state.clone(),
            gpu_list: context.gpu_list.clone(),
            config: context.config.clone(),
            blocker: context.blocker.clone(),
            pci_list: context.pci_list.clone(),
            object_server,
            power_tasks: context.power_tasks.clone(),
            switcheroo,
        })
    }

    /// Reconcile CW_EXP_BLK_INO with the nvidia files currently on disk
    ///
    /// Missing inodes only warn, the map keeps what it holds. A failed map
    /// write is returned: the block would be advertised but not enforced
    pub async fn sync_nvidia_inodes(&self) -> Result<()> {
        let target = {
            let gpu_list = self.gpu_list.read().await;
            gpu_list
                .iter()
                .find(|(_, gpu)| {
                    gpu.device.gpu_vendor() == GpuVendor::Nvidia && !gpu.device.is_default()
                })
                .map(|(id, _)| *id as u32)
        };

        let inodes = match target {
            Some(_) => match exp_nvidia_inodes() {
                Ok(inodes) => inodes,
                Err(err) => {
                    warn!(
                        "failed to read nvidia inodes, leaving the map as is: {:#}",
                        err
                    );
                    return Ok(());
                }
            },
            None => Vec::new(),
        };

        let mut blocker = self.blocker.write().await;
        match target {
            Some(gpu_id) => blocker.sync_exp_inodes(inodes, gpu_id),
            None => blocker.clear_exp_inodes(),
        }
        .map_err(|err| err.into())
    }

    async fn drop_unclaimed_inodes(&self, previous: Vec<InodeKey>) {
        let claimed: HashSet<InodeKey> = {
            let gpu_interfaces = self.gpu_list.read().await;
            let mut claimed = HashSet::new();
            for gpu in gpu_interfaces.values() {
                claimed.extend(gpu.pushed_inodes().await);
            }
            claimed
        };

        let mut blocker = self.blocker.write().await;
        for stale in previous.iter().filter(|key| !claimed.contains(key)) {
            if let Err(err) = blocker.remove_inode(*stale) {
                warn!("failed to drop stale inode {:?}: {}", stale, err);
            }
        }
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Debug")]
impl DebugInterface {
    pub async fn get_pci_devices(&self) -> fdo::Result<BTreeMap<String, DbusPciDevice>> {
        let pci_list = &self.pci_list.read().await;
        let mut dbus_list: BTreeMap<String, DbusPciDevice> = BTreeMap::new();
        for (id, pci) in pci_list.iter() {
            dbus_list.insert(id.clone(), DbusPciDevice::from(pci));
        }
        Ok(dbus_list)
    }
    pub async fn refresh_gpu(&self) -> fdo::Result<()> {
        // read a new pci list, if it's different than the current one, refresh the gpus, else do
        // nothing
        let new_pci_list =
            pci::read_pci_devices().map_err(|err| fdo::Error::Failed(err.to_string()))?;
        let mut pci_list = self.pci_list.write().await;
        let changed = new_pci_list != *pci_list;
        if changed && let Some(object_server) = &self.object_server {
            info!("pci list changed, refreshing the internal gpu list");
            // Overwrite old list, drop the lock before the blocking rebuild
            *pci_list = new_pci_list.clone();
            drop(pci_list);

            // lock the importants components
            let mut gpu_interfaces = self.gpu_list.write().await;
            let mut power_tasks = self.power_tasks.write().await;

            // get rid of the old gpu api and the old tasks
            let mut previous_inodes: Vec<InodeKey> = Vec::new();
            for (id, gpu) in gpu_interfaces.iter() {
                let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
                let _ = object_server.remove::<GpuInterface, &str>(&path).await;
                // if task is present, abort
                if let Some(handle) = power_tasks.remove(id) {
                    handle.abort();
                }
                previous_inodes.extend(gpu.take_pushed_inodes().await);
            }

            // Empty the current gpu_interfaces
            gpu_interfaces.clear();
            // Read the new list. The blocking enumeration is intentional: the gpu_list write lock
            // until the new list is complete
            let gpu_enumator = GpuEnumerator::build();
            let new_gpu_list = gpu_enumator.enumerate(&new_pci_list);
            let gpu_count = new_gpu_list
                .iter()
                .filter(|(_, gpu)| gpu.is_available())
                .count();
            for (id, device) in new_gpu_list {
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
                    Arc::clone(&self.blocker),
                    Arc::clone(&self.pci_list),
                    Arc::clone(&self.gpu_state),
                    Arc::clone(&self.mode_state),
                    self.switcheroo.clone(),
                )
                .map_err(|err| fdo::Error::Failed(err.to_string()))?;

                gpu_interfaces.insert(id, Arc::new(gpu));
            }

            // now re-populate the gpu api
            for (id, gpu_interface) in gpu_interfaces.iter() {
                let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
                object_server
                    .at(path.clone(), gpu_interface.as_ref().clone())
                    .await?;
                let gpu_ref = object_server
                    .interface::<_, GpuInterface>(path)
                    .await
                    .map_err(|err| fdo::Error::Failed(err.to_string()))?;
                gpu_interface
                    .signal_emitter
                    .get_or_init(|| gpu_ref.signal_emitter().to_owned());
                // spawn power state tasks only for available GPUs
                if gpu_interface.device.is_available() {
                    let handle = task::spawn(watch_power_state(Arc::clone(gpu_interface), gpu_ref));
                    power_tasks.insert(*id, handle);
                }
            }

            drop(power_tasks);
            drop(gpu_interfaces);

            // Re-apply the persisted mode against the new GPU list
            let requested = match CardwireModeState::build() {
                Ok(state) => state.mode(),
                Err(e) => {
                    warn!("failed to read persisted mode on hotplug, using hybrid: {e}");
                    Modes::Hybrid
                }
            };
            if let Err(e) = self
                .mode_interface
                .internal_set_mode(requested, false)
                .await
            {
                warn!("failed to re-apply mode on hotplug, falling back to hybrid: {e}");
                if let Err(fb) = self
                    .mode_interface
                    .internal_set_mode(Modes::Hybrid, false)
                    .await
                {
                    warn!("failed to fall back to hybrid mode on hotplug: {fb}");
                }
            }

            self.drop_unclaimed_inodes(previous_inodes).await;
            if let Err(err) = self.sync_nvidia_inodes().await {
                error!(
                    "nvidia block is out of date after the gpu refresh: {:#}",
                    err
                );
            }
            self.switcheroo.emit_gpu_list_changed().await;
        }

        Ok(())
    }
}
