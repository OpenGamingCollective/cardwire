//! this is a middleman between the daemon and the ebpf library
use std::collections::BTreeMap;

use crate::{errors::Error as CardwireError, gpu::models::GpuDevice, pci::PciDevice};
use cardwire_ebpf::EbpfBlocker;

pub struct GpuBlocker {
    inner: EbpfBlocker,
}

impl GpuBlocker {
    pub fn new() -> Result<Self, CardwireError> {
        Ok(Self {
            inner: EbpfBlocker::new()?,
        })
    }

    pub fn set_vulkan_block(&mut self, block: bool) -> Result<(), CardwireError> {
        self.inner
            .set_vulkan_block(block)
            .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?;
        Ok(())
    }
    pub fn set_file_block(&mut self, file: &str) -> Result<(), CardwireError> {
        self.inner
            .set_file_block(file)
            .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?;
        Ok(())
    }
    pub fn set_nvidia_file_block(&mut self, file: &str) -> Result<(), CardwireError> {
        self.inner
            .set_nvidia_file_block(file)
            .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?;
        Ok(())
    }
}

pub fn is_gpu_blocked(blocker: &GpuBlocker, gpu: &GpuDevice) -> Result<bool, CardwireError> {
    let card_id = *gpu.card();
    let render_id = *gpu.render();
    Ok(blocker
        .inner
        .is_pci_blocked(gpu.pci.pci_address())
        .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?
        && blocker
            .inner
            .is_card_blocked(card_id)
            .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?
        && blocker
            .inner
            .is_render_blocked(render_id)
            .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?
        && if gpu.nvidia() {
            // unwrap because it should be Some if it's an nvidia gpu, if not it's a bug and should
            // be reported
            blocker
                .inner
                .is_nvidia_blocked(gpu.nvidia_minor().unwrap())
                .map_err(|err| CardwireError::UnknownBlockState(err.to_string()))?
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
        blocker.inner.block_card(card_id)?;
        blocker.inner.block_render(render_id)?;
        recursive_block_pci(blocker, gpu, pci_list)?;
        if gpu.nvidia() {
            blocker.inner.block_nvidia(gpu.nvidia_minor().unwrap())?
        }
        Ok(())
    } else {
        blocker.inner.unblock_card(card_id)?;
        blocker.inner.unblock_render(render_id)?;
        recursive_unblock_pci(blocker, gpu, pci_list)?;
        if gpu.nvidia() {
            blocker.inner.unblock_nvidia(gpu.nvidia_minor().unwrap())?
        }
        Ok(())
    }
}

fn recursive_block_pci(
    blocker: &mut GpuBlocker,
    gpu: &GpuDevice,
    pci_list: &BTreeMap<String, PciDevice>,
) -> Result<(), CardwireError> {
    // Block the gpu pci
    blocker.inner.block_pci(gpu.pci.pci_address())?;
    // also block audio card
    if gpu.pci.pci_address().ends_with(".0") {
        let gpu_audio_adress = gpu.pci.pci_address().to_string().replace(".0", ".1");
        blocker.inner.block_pci(&gpu_audio_adress)?;
    }
    // Check if gpu has a parent pci
    if gpu.pci.parent_pci().is_some() {
        // first pci to block
        let mut parent_pci: String = gpu.pci.parent_pci().clone().unwrap();
        loop {
            // Also block the parent pci
            if let Some(pci_device) = pci_list.get(&parent_pci) {
                blocker.inner.block_pci(pci_device.pci_address())?;
                if pci_device.parent_pci().is_some() {
                    // if the device contain a parent, continue the loop
                    parent_pci = pci_device.parent_pci().clone().unwrap()
                } else {
                    // if no parent. exit the loop
                    break;
                }
            }
        }
    }
    Ok(())
}
fn recursive_unblock_pci(
    blocker: &mut GpuBlocker,
    gpu: &GpuDevice,
    pci_list: &BTreeMap<String, PciDevice>,
) -> Result<(), CardwireError> {
    // Unblock the gpu pci
    blocker.inner.unblock_pci(gpu.pci.pci_address())?;
    // also unblock audio card
    if gpu.pci.pci_address().ends_with(".0") {
        let gpu_audio_adress = gpu.pci.pci_address().to_string().replace(".0", ".1");
        blocker.inner.block_pci(&gpu_audio_adress)?;
    }
    // Check if gpu has a parent pci
    if gpu.pci.parent_pci().is_some() {
        // first pci to block
        let mut parent_pci: String = gpu.pci.parent_pci().clone().unwrap();
        loop {
            // Also block the parent pci
            if let Some(pci_device) = pci_list.get(&parent_pci) {
                blocker.inner.unblock_pci(pci_device.pci_address())?;
                if pci_device.parent_pci().is_some() {
                    // if the device contain a parent, continue the loop
                    parent_pci = pci_device.parent_pci().clone().unwrap()
                } else {
                    // if no parent. exit the loop
                    break;
                }
            }
        }
    }
    Ok(())
}
