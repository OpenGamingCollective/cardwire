use std::{collections::BTreeMap, sync::Arc};

use cardwire_core::{
    gpu::{GpuBlocker, GpuDevice, block_gpu, is_gpu_blocked}, pci::PciDevice
};
use tokio::sync::RwLock;
use zbus::{fdo, interface};

#[derive(Clone)]
pub struct Gpu {
    pub device: GpuDevice,
    blocker: Arc<RwLock<GpuBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    blocked: bool,
}

#[derive(Clone)]
pub struct GpuState {
    pub inner: Arc<RwLock<Gpu>>,
}

impl GpuState {
    pub fn new(
        blocker: Arc<RwLock<GpuBlocker>>,
        device: GpuDevice,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Gpu {
                device,
                blocker,
                pci_list,
                blocked: false,
            })),
        }
    }
}
impl Gpu {
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

#[interface(name = "com.github.opengamingcollective.cardwire.gpu")]
impl GpuState {
    #[zbus(property)]
    pub async fn set_block(&mut self, block: bool) -> fdo::Result<()> {
        let mut state = self.inner.write().await;
        if block {
            state.block_gpu().await
        } else {
            state.unblock_gpu().await
        }
    }

    #[zbus(property)]
    pub async fn block(&self) -> fdo::Result<bool> {
        let state = self.inner.read().await;
        Ok(state.blocked())
    }
}
