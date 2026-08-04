use crate::{
    core::{
        gpu::GpuEnumerator, pci::{self, DbusPciDevice, PciDevice}
    }, tasks::watch_power_state
};
use cardwire_ebpf_userspace::EbpfBlocker;
use log::{info, warn};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::RwLock, task};
use zbus::{fdo, interface};

use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{ConfigMemory, GpuInterface, ModeInterface, Modes}
};

#[derive(Clone)]
pub struct DebugInterface {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub mode_interface: ModeInterface,
    pub gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config: Arc<ConfigMemory>,
    pub blocker: Arc<RwLock<EbpfBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    pub object_server: Option<zbus::ObjectServer>,
    pub power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<anyhow::Result<()>>>>>,
}
impl DebugInterface {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        mode_interface: ModeInterface,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<EbpfBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
        object_server: Option<zbus::ObjectServer>,
        power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<anyhow::Result<()>>>>>,
    ) -> anyhow::Result<DebugInterface> {
        Ok(DebugInterface {
            mode_state,
            mode_interface,
            gpu_state,
            gpu_list,
            config,
            blocker,
            pci_list,
            object_server,
            power_tasks,
        })
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Debug")]
impl DebugInterface {
    pub async fn get_pci_devices(&self) -> fdo::Result<BTreeMap<String, DbusPciDevice>> {
        let pci_list = &self.pci_list.read().await;
        let mut dbus_list: BTreeMap<String, DbusPciDevice> = BTreeMap::new();
        for (id, pci) in pci_list.iter() {
            let temp_pci = DbusPciDevice {
                iommu_group: if let Some(iommu) = pci.iommu_group() {
                    iommu.to_string()
                } else {
                    "".to_string()
                },
                vendor_id: pci.vendor_id().clone().unwrap_or("".to_string()),
                device_id: pci.device_id().clone().unwrap_or("".to_string()),
                vendor_name: pci.vendor_name().clone().unwrap_or("".to_string()),
                device_name: pci.device_name().clone().unwrap_or("".to_string()),
                driver: pci.driver().clone().unwrap_or("".to_string()),
                class: pci.class().clone().unwrap_or("".to_string()),
                parent_pci: pci.parent_pci().clone().unwrap_or("".to_string()),
                child_pci: pci.child_pci().clone().unwrap_or("".to_string()),
            };
            dbus_list.insert(id.clone(), temp_pci);
        }

        Ok(dbus_list)
    }
    pub async fn refresh_gpu(&self) -> fdo::Result<()> {
        // lock the importants components
        let mut pci_list = self.pci_list.write().await;
        let mut gpu_interfaces = self.gpu_list.write().await;

        // read a new pci list, if it's different than the current one, refresh the gpus, else do
        // nothing
        let new_pci_list =
            pci::read_pci_devices().map_err(|err| fdo::Error::Failed(err.to_string()))?;
        if new_pci_list != *pci_list
            && let Some(object_server) = &self.object_server
        {
            info!("pci list changed, refreshing the internal gpu list");
            // Overwrite old list
            *pci_list = new_pci_list.clone();
            drop(pci_list); // drop lock to prevent deadlocks when blocking

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
            // Read the new list
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

                gpu_interfaces.insert(id, gpu);
            }

            // now re-populate the gpu api
            for (id, gpu_interface) in gpu_interfaces.iter() {
                let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
                object_server
                    .at(path.clone(), gpu_interface.clone())
                    .await?;
                // spawn power state tasks only for available GPUs
                if gpu_interface.device.is_available() {
                    let handle = task::spawn(watch_power_state(
                        gpu_interface.clone(),
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

            // Rebuild hotplug state from the effective mode. Resolve the display target first so
            // a hotplug never blocks a GPU that is driving a connected display. If anything fails
            // here the eBPF block map is left stale, so fall back to hybrid (unblocks every GPU)
            // instead of manual: manual can re-block a GPU from a saved gpu_state without a
            // display guard.
            let mode = self.mode_interface.current_mode_value().await;
            let target = match self.mode_interface.detect_display_target(mode).await {
                Ok((target, _)) => target,
                Err(err) => {
                    warn!(
                        "failed to resolve display target on hotplug, falling back to hybrid: {err}"
                    );
                    if let Err(fb) = self
                        .mode_interface
                        .effective_set_mode(Modes::Hybrid, true)
                        .await
                    {
                        warn!("failed to fall back to hybrid mode on hotplug: {fb}");
                    }
                    return Err(err);
                }
            };
            if let Err(e) = self.mode_interface.effective_set_mode(target, true).await {
                warn!("failed to re-apply mode on hotplug, falling back to hybrid: {e}");
                if let Err(fb) = self
                    .mode_interface
                    .effective_set_mode(Modes::Hybrid, true)
                    .await
                {
                    warn!("failed to fall back to hybrid mode on hotplug: {fb}");
                }
            }
        }

        Ok(())
    }
}
