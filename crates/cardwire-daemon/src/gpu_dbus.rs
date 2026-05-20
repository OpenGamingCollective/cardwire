use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Error};
use cardwire_core::{
    gpu::{GpuBlocker, GpuDevice, block_gpu, is_gpu_blocked}, pci::PciDevice
};
use tokio::sync::RwLock;
use zbus::{fdo, interface};

pub struct Gpu {
    pub device: GpuDevice,
    blocker: Arc<RwLock<GpuBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    blocked: bool,
}

impl Gpu {
    pub fn new(
        blocker: Arc<RwLock<GpuBlocker>>,
        device: GpuDevice,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    ) -> Self {
        Self {
            device,
            blocker,
            pci_list,
            blocked: false,
        }
    }

    // block the gpu
    pub async fn block_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let gpu = &self.device;
        let pci_list = self.pci_list.read().await;

        println!("blocking gpu {}, with {}", gpu.name(), true);
        block_gpu(&mut blocker, gpu, true, &pci_list)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        if let Ok(result) = is_gpu_blocked(&blocker, &gpu) {
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
        let gpu = &self.device;
        let pci_list = self.pci_list.read().await;

        block_gpu(&mut blocker, gpu, false, &pci_list)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        if let Ok(result) = is_gpu_blocked(&blocker, &gpu) {
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

impl Gpu {
    #[zbus(property)]
    pub async fn set_block(&self, block: bool) -> fdo::Result<()> {
        Ok(())
    }

    #[zbus(property)]
    pub async fn block(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
