//! DBUS Interface for single gpu interaction

use std::{collections::BTreeMap, sync::Arc};

use cardwire_core::{
    gpu::{GpuBlocker, GpuDevice, block_gpu, is_gpu_blocked}, pci::PciDevice
};
use tokio::sync::RwLock;
use zbus::{fdo, interface};

// Represent a single gpu
#[derive(Clone)]
pub struct GpuInterface {
    pub device: GpuDevice,
    blocker: Arc<RwLock<GpuBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    blocked: bool,
}

impl GpuInterface {
    pub fn build(
        device: GpuDevice,
        blocker: Arc<RwLock<GpuBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    ) -> anyhow::Result<GpuInterface> {
        Ok(Self {
            device,
            blocker,
            pci_list,
            blocked: false,
        })
    }
}

impl GpuInterface {
    // block the gpu
    pub async fn block_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let pci_list = self.pci_list.read().await;
        block_gpu(&mut blocker, &self.device, true, &pci_list)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        if let Ok(result) = is_gpu_blocked(&blocker, &self.device) {
            if !result {
                return Err(fdo::Error::Failed(
                    "gpu is supposed to be blocked, bpf says it's not".to_string(),
                ));
            } else {
                self.blocked = result;
            }
        }
        Ok(())
    }
    // unblock the gpu
    pub async fn unblock_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let pci_list = self.pci_list.read().await;

        block_gpu(&mut blocker, &self.device, false, &pci_list)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        if let Ok(result) = is_gpu_blocked(&blocker, &self.device) {
            if result {
                return Err(fdo::Error::Failed(
                    "gpu is supposed to be unblocked, bpf says it's not".to_string(),
                ));
            } else {
                self.blocked = result;
            }
        }
        Ok(())
    }
    pub fn blocked(&self) -> bool {
        self.blocked
    }
}

#[interface(
    name = "com.github.opengamingcollective.Gpu",
    proxy(
        default_service = "com.github.opengamingcollective.cardwire",
        default_path = "/com/github/opengamingcollective/cardwire"
    )
)]
impl GpuInterface {
    #[zbus(property)]
    pub async fn set_block(&mut self, block: bool) -> fdo::Result<()> {
        if block {
            self.block_gpu().await
        } else {
            self.unblock_gpu().await
        }
    }

    #[zbus(property)]
    pub async fn block(&self) -> fdo::Result<bool> {
        Ok(self.blocked())
    }
}
