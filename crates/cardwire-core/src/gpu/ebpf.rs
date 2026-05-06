//! this is a middleman between the daemon and the ebpf library
use crate::{
    errors::{CardwireCoreError, CardwireCoreResult}, gpu::models::GpuDevice
};
use cardwire_ebpf::EbpfBlocker;

pub struct GpuBlocker {
    inner: EbpfBlocker,
}

impl GpuBlocker {
    pub fn new() -> CardwireCoreResult<Self> {
        Ok(Self {
            inner: EbpfBlocker::new()?,
        })
    }

    pub fn set_vulkan_block(&mut self, block: bool) -> CardwireCoreResult<()> {
        self.inner.set_vulkan_block(block).map_err(map_gpu_error)?;
        Ok(())
    }
    pub fn set_file_block(&mut self, file: &str) -> CardwireCoreResult<()> {
        self.inner.set_file_block(file).map_err(map_gpu_error)?;
        Ok(())
    }
    pub fn set_nvidia_file_block(&mut self, file: &str) -> CardwireCoreResult<()> {
        self.inner
            .set_nvidia_file_block(file)
            .map_err(map_gpu_error)?;
        Ok(())
    }
}

pub fn is_gpu_blocked(blocker: &GpuBlocker, gpu: &GpuDevice) -> CardwireCoreResult<bool> {
    let (card_id, render_id) = gpu_node_ids(gpu).map_err(map_gpu_error)?;
    Ok(blocker
        .inner
        .is_pci_blocked(gpu.pci.pci_address())
        .map_err(map_gpu_error)?
        && blocker
            .inner
            .is_card_blocked(card_id)
            .map_err(map_gpu_error)?
        && blocker
            .inner
            .is_render_blocked(render_id)
            .map_err(map_gpu_error)?
        && if gpu.nvidia() {
            // unwrap because it should be Some if it's an nvidia gpu, if not it's a bug and should
            // be reported
            blocker
                .inner
                .is_nvidia_blocked(gpu.nvidia_minor().unwrap())
                .map_err(map_gpu_error)?
        } else {
            true
        })
}

pub fn block_gpu(blocker: &mut GpuBlocker, gpu: &GpuDevice, block: bool) -> CardwireCoreResult<()> {
    let (card_id, render_id) = gpu_node_ids(gpu)?;

    if block {
        blocker.inner.block_card(card_id)?;
        blocker.inner.block_render(render_id)?;
        blocker.inner.block_pci(gpu.pci.pci_address())?;
        if gpu.nvidia() {
            blocker.inner.block_nvidia(gpu.nvidia_minor().unwrap())?
        }
        Ok(())
    } else {
        blocker.inner.unblock_card(card_id)?;
        blocker.inner.unblock_render(render_id)?;
        blocker.inner.unblock_pci(gpu.pci.pci_address())?;
        if gpu.nvidia() {
            blocker.inner.unblock_nvidia(gpu.nvidia_minor().unwrap())?
        }
        Ok(())
    }
}

fn gpu_node_ids(gpu: &GpuDevice) -> CardwireCoreResult<(u32, u32)> {
    let card_id = *gpu.card();
    let render_id = *gpu.render();
    Ok((card_id, render_id))
}

fn map_gpu_error(err: impl std::fmt::Display) -> CardwireCoreError {
    CardwireCoreError::UnknownBlockState(err.to_string())
}
