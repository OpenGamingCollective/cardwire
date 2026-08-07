use crate::{
    core::{
        gpu::GpuEnumerator, pci::{self, DbusPciDevice, PciDevice}
    }, interface::SwitcherooInterface, tasks::watch_power_state
};
use cardwire_ebpf_userspace::EbpfBlocker;
use log::{info, warn};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::RwLock, task};
use zbus::{fdo, interface};

use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{ConfigMemory, DaemonContext, GpuInterface, ModeInterface, Modes}
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
    pub power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<anyhow::Result<()>>>>>,
    pub switcheroo: SwitcherooInterface,
}
impl DebugInterface {
    pub fn build(
        context: &DaemonContext,
        mode_interface: ModeInterface,
        object_server: Option<zbus::ObjectServer>,
        switcheroo: SwitcherooInterface,
    ) -> anyhow::Result<DebugInterface> {
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
        // nothing. The sysfs scan runs without holding any lock.
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
            for id in gpu_interfaces.keys() {
                let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
                let _ = object_server.remove::<GpuInterface, &str>(&path).await;
                // if task is present, abort
                if let Some(handle) = power_tasks.remove(id) {
                    handle.abort();
                }
            }

            // Empty the current gpu_interfaces
            gpu_interfaces.clear();
            // Read the new list. The blocking enumeration is intentional: the gpu_list write lock
            // serializes readers until the new list is complete, and the multi-threaded runtime
            // absorbs the stall
            let gpu_enumator = GpuEnumerator::build();
            let new_gpu_list = gpu_enumator.enumerate(&new_pci_list);
            for (id, device) in new_gpu_list {
                let gpu = GpuInterface::build(
                    id as u32,
                    device,
                    Arc::clone(&self.blocker),
                    Arc::clone(&self.pci_list),
                    Arc::clone(&self.gpu_state),
                    Arc::clone(&self.mode_state),
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
                // spawn power state tasks only for available GPUs
                if gpu_interface.device.is_available() {
                    let handle = task::spawn(watch_power_state(
                        Arc::clone(gpu_interface),
                        object_server
                            .interface(path)
                            .await
                            .map_err(|err| fdo::Error::Failed(err.to_string()))?,
                    ));
                    power_tasks.insert(*id, handle);
                }
            }

            drop(power_tasks);
            drop(gpu_interfaces);

            // Re-apply the persisted mode against the new GPU list.
            let requested = self.mode_state.read().await.mode();
            if let Err(e) = self
                .mode_interface
                .internal_set_mode(requested, Some(false))
                .await
            {
                warn!("failed to re-apply mode on hotplug, falling back to hybrid: {e}");
                if let Err(fb) = self
                    .mode_interface
                    .internal_set_mode(Modes::Hybrid, Some(false))
                    .await
                {
                    warn!("failed to fall back to hybrid mode on hotplug: {fb}");
                }
            }
            self.switcheroo.emit_gpu_list_changed().await;
        }

        Ok(())
    }
}
