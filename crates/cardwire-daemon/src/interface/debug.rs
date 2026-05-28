use cardwire_core::{
    gpu::GpuBlocker, pci::{DbusPciDevice, PciDevice}
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
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
    pub blocker: Arc<RwLock<GpuBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
}
impl DebugInterface {
    pub fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<GpuBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    ) -> anyhow::Result<DebugInterface> {
        Ok(DebugInterface {
            mode_state,
            gpu_state,
            gpu_list,
            config,
            blocker,
            pci_list,
        })
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Debug")]
impl DebugInterface {
    pub(crate) async fn get_pci_devices(&self) -> fdo::Result<BTreeMap<String, DbusPciDevice>> {
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
}
