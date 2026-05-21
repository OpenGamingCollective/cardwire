//! DBUS Interface for single gpu interaction

use std::{collections::BTreeMap, sync::Arc};

use cardwire_core::{
    gpu::{GpuBlocker, GpuDevice, block_gpu, is_gpu_blocked}, pci::PciDevice
};
use log::{info, warn};
use tokio::sync::RwLock;
use zbus::{fdo, interface};

use crate::file::CardwireGpuState;

// Represent a single gpu
#[derive(Clone)]
pub struct GpuInterface {
    pub device: GpuDevice,
    blocker: Arc<RwLock<GpuBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
}

impl GpuInterface {
    pub fn build(
        device: GpuDevice,
        blocker: Arc<RwLock<GpuBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
    ) -> anyhow::Result<GpuInterface> {
        Ok(Self {
            device,
            blocker,
            pci_list,
            gpu_state,
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
        if let Ok(result) = is_gpu_blocked(&blocker, &self.device)
            && !result
        {
            return Err(fdo::Error::Failed(
                "gpu is supposed to be blocked, bpf says it's not".to_string(),
            ));
        };
        Ok(())
    }
    // unblock the gpu
    pub async fn unblock_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let pci_list = self.pci_list.read().await;

        block_gpu(&mut blocker, &self.device, false, &pci_list)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        if let Ok(result) = is_gpu_blocked(&blocker, &self.device)
            && result
        {
            return Err(fdo::Error::Failed(
                "gpu is supposed to be unblocked, bpf says it's not".to_string(),
            ));
        };
        Ok(())
    }
    pub async fn gpu_blocked(&self) -> fdo::Result<bool> {
        let blocker = self.blocker.read().await;
        is_gpu_blocked(&blocker, &self.device).map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Gpu")]
impl GpuInterface {
    #[zbus(property)]
    pub async fn set_block(&mut self, block: bool) -> fdo::Result<()> {
        if block {
            // Don't block if default
            if self.device.is_default() {
                return Err(fdo::Error::AccessDenied(format!(
                    "GPU {} is the default device and cannot be blocked",
                    self.device.name()
                )));
            }
            // Now block
            self.block_gpu().await?;
            info!("Set GPU {} block={}", self.device.name(), block);
            // save new state to file
            let mut gpu_state = self.gpu_state.write().await;
            if let Err(e) = gpu_state.save_state(&self.device, true).await {
                warn!("could not save gpu_state to file: {e}");
            };
            Ok(())
        } else {
            // unblock
            self.unblock_gpu().await?;
            info!("Set GPU {} block={}", self.device.name(), block);
            // save new state to file
            let mut gpu_state = self.gpu_state.write().await;
            if let Err(e) = gpu_state.save_state(&self.device, false).await {
                warn!("could not save gpu_state to file: {e}");
            };
            Ok(())
        }
    }

    #[zbus(property)]
    pub async fn block(&self) -> fdo::Result<bool> {
        self.gpu_blocked().await
    }
}
