use crate::{
    core::{
        gpu::{self, check_default_drm_class}, pci::{self, DbusPciDevice, PciDevice}
    }, tasks::watch_power_state
};
use cardwire_ebpf::EbpfBlocker;
use log::{info, warn};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::RwLock, task};
use zbus::{fdo, interface};

use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{ConfigMemory, GpuInterface}
};

#[derive(Clone)]
pub struct DebugInterface {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
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

#[interface(name = "com.github.opengamingcollective.cardwire.Debug")]
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
            let mut power_tasks = self.power_tasks.write().await;

            // get rid of the old gpu api and the old tasks
            for (id, _) in gpu_interfaces.iter() {
                let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
                let _ = object_server.remove::<GpuInterface, &str>(&path).await;
                // if task is present, abort
                if let Some(handle) = power_tasks.remove(id) {
                    handle.abort();
                }
            }

            // Empty the current gpu_interfaces
            gpu_interfaces.clear();
            // Read the new list
            let mut new_gpu_list =
                gpu::read_gpu(&pci_list).map_err(|err| fdo::Error::Failed(err.to_string()))?;
            if let Err(err) = check_default_drm_class(&mut new_gpu_list) {
                warn!("Failed to determine default GPU: {}", err);
            }
            for (id, device) in new_gpu_list {
                let gpu = GpuInterface::build(
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
                let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
                object_server
                    .at(path.clone(), gpu_interface.clone())
                    .await?;
                // spawn power state tasks
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

        Ok(())
    }
}
