//! this is a middleman between the daemon and the ebpf library
use std::collections::BTreeMap;

use crate::{errors::Error as CardwireError, gpu::models::GpuDevice, pci::PciDevice};
use cardwire_ebpf::{BlockKind, EbpfBlocker};
use log::{info, warn};

pub struct GpuBlocker {
    inner: EbpfBlocker,
}

impl GpuBlocker {
    pub fn new() -> Result<Self, CardwireError> {
        Ok(Self {
            inner: EbpfBlocker::new()?,
        })
    }

    pub fn set_nvidia_setting(&mut self, block: bool) -> Result<(), CardwireError> {
        if block {
            self.inner
                .block_kind(&block.to_string(), BlockKind::NvidiaSetting)?;
        } else {
            self.inner
                .unblock_kind(&block.to_string(), BlockKind::NvidiaSetting)?;
        }
        Ok(())
    }
    pub fn set_file_block(&mut self, file: &str) -> Result<(), CardwireError> {
        self.inner.block_kind(file, BlockKind::File)?;
        Ok(())
    }
    pub fn set_nvidia_file_block(&mut self, file: &str) -> Result<(), CardwireError> {
        self.inner.block_kind(file, BlockKind::NvidiaFile)?;
        Ok(())
    }
}

pub fn is_gpu_blocked(blocker: &GpuBlocker, gpu: &GpuDevice) -> Result<bool, CardwireError> {
    let card_id = *gpu.card();
    let render_id = *gpu.render();
    // PCI -> Card -> Render -> Nvidia
    Ok(blocker
        .inner
        .is_kind_blocked(gpu.pci.pci_address(), BlockKind::Pci)?
        && blocker
            .inner
            .is_kind_blocked(&card_id.to_string(), BlockKind::Card)?
        && blocker
            .inner
            .is_kind_blocked(&render_id.to_string(), BlockKind::Render)?
        && if let Some(minor) = gpu.nvidia_minor() {
            blocker
                .inner
                .is_kind_blocked(&minor.to_string(), BlockKind::Nvidia)?
        } else {
            true
        })
}

pub fn block_gpu(
    blocker: &mut GpuBlocker,
    gpu: &GpuDevice,
    block: bool,
    pci_list: &BTreeMap<String, PciDevice>,
) -> Result<(), CardwireError> {
    let card_id = *gpu.card();
    let render_id = *gpu.render();

    if block {
        //blocker.inner.block_card(card_id)?;
        // block card
        blocker
            .inner
            .block_kind(&card_id.to_string(), BlockKind::Card)?;
        // block render
        blocker
            .inner
            .block_kind(&render_id.to_string(), BlockKind::Render)?;
        // block pci
        chain_block_pci(blocker, gpu, pci_list)?;
        // block nvidia
        if gpu.nvidia() {
            blocker
                .inner
                .block_kind(&gpu.nvidia_minor().unwrap().to_string(), BlockKind::Nvidia)?;
        }
        Ok(())
    } else {
        // unblock card
        blocker
            .inner
            .unblock_kind(&card_id.to_string(), BlockKind::Card)?;
        // unblock render
        blocker
            .inner
            .unblock_kind(&render_id.to_string(), BlockKind::Render)?;
        // unblock pci
        chain_unblock_pci(blocker, gpu, pci_list)?;
        // unblock nvidia
        if gpu.nvidia() {
            blocker
                .inner
                .unblock_kind(&gpu.nvidia_minor().unwrap().to_string(), BlockKind::Nvidia)?;
        }
        Ok(())
    }
}

fn chain_block_pci(
    blocker: &mut GpuBlocker,
    gpu: &GpuDevice,
    pci_list: &BTreeMap<String, PciDevice>,
) -> Result<(), CardwireError> {
    // Block the gpu pci
    blocker
        .inner
        .block_kind(gpu.pci.pci_address(), BlockKind::Pci)?;
    info!("blocking pci: {}", gpu.pci.pci_address());
    // also block audio card
    if gpu.pci.pci_address().ends_with(".0") {
        let gpu_audio_adress = gpu.pci.pci_address().replace(".0", ".1");
        blocker
            .inner
            .block_kind(&gpu_audio_adress, BlockKind::Pci)?;
    }
    // Check if gpu has a parent pci
    // first pci to block
    let mut current_parent = gpu.pci.parent_pci().clone();

    while let Some(parent_pci) = current_parent {
        if let Some(pci_device) = pci_list.get(&parent_pci) {
            info!("chain blocking pci: {}", pci_device.pci_address());
            blocker
                .inner
                .block_kind(pci_device.pci_address(), BlockKind::Pci)?;
            current_parent = pci_device.parent_pci().clone();
        } else {
            warn!("expected parent pci {} not found in pci_list", parent_pci);
            break;
        }
    }
    Ok(())
}
fn chain_unblock_pci(
    blocker: &mut GpuBlocker,
    gpu: &GpuDevice,
    pci_list: &BTreeMap<String, PciDevice>,
) -> Result<(), CardwireError> {
    // Unblock the gpu pci
    info!("unblocking pci: {}", gpu.pci.pci_address());
    blocker
        .inner
        .unblock_kind(gpu.pci.pci_address(), BlockKind::Pci)?;

    // also unblock audio card
    if gpu.pci.pci_address().ends_with(".0") {
        let gpu_audio_adress = gpu.pci.pci_address().to_string().replace(".0", ".1");
        blocker
            .inner
            .unblock_kind(&gpu_audio_adress, BlockKind::Pci)?;
    }
    // Check if gpu has a parent pci
    // first pci to block
    let mut current_parent = gpu.pci.parent_pci().clone();

    while let Some(parent_pci) = current_parent {
        if let Some(pci_device) = pci_list.get(&parent_pci) {
            info!("chain unblocking pci: {}", pci_device.pci_address());
            blocker
                .inner
                .unblock_kind(pci_device.pci_address(), BlockKind::Pci)?;
            current_parent = pci_device.parent_pci().clone();
        } else {
            warn!("expected parent pci {} not found in pci_list", parent_pci);
            break;
        }
    }
    Ok(())
}
