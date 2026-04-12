use crate::gpu::{GpuResult, errors::GpuError, models::Gpu};
use cardwire_ebpf::EbpfBlocker;
use std::{
    io::{Error as IoError, ErrorKind}, path::Path
};

pub struct GpuBlocker {
    inner: EbpfBlocker,
}

impl GpuBlocker {
    pub fn new() -> GpuResult<Self> {
        Ok(Self {
            inner: EbpfBlocker::new()?,
        })
    }

    pub fn set_vulkan_block(&mut self, block: bool) -> GpuResult<()> {
        self.inner.set_vulkan_block(block).map_err(map_gpu_error)?;
        Ok(())
    }
}

pub fn is_gpu_blocked(blocker: &GpuBlocker, gpu: &Gpu) -> GpuResult<bool> {
    let (card_id, render_id) = gpu_node_ids(gpu).map_err(map_gpu_error)?;
    Ok(blocker
        .inner
        .is_pci_blocked(gpu.pci_address())
        .map_err(map_gpu_error)?
        && blocker
            .inner
            .is_card_blocked(card_id)
            .map_err(map_gpu_error)?
        && blocker
            .inner
            .is_render_blocked(render_id)
            .map_err(map_gpu_error)?
        && if gpu.nvidia {
            blocker
                .inner
                .is_nvidia_blocked(*gpu.nvidia_minor())
                .map_err(map_gpu_error)?
        } else {
            true
        })
}

pub fn block_gpu(blocker: &mut GpuBlocker, gpu: &Gpu, block: bool) -> GpuResult<()> {
    let (card_id, render_id) = gpu_node_ids(gpu)?;

    if block {
        blocker.inner.block_card(card_id)?;
        blocker.inner.block_render(render_id)?;
        blocker.inner.block_pci(gpu.pci_address())?;
        if gpu.nvidia {
            blocker.inner.block_nvidia(*gpu.nvidia_minor())?
        }
        Ok(())
    } else {
        blocker.inner.unblock_card(card_id)?;
        blocker.inner.unblock_render(render_id)?;
        blocker.inner.unblock_pci(gpu.pci_address())?;
        if gpu.nvidia {
            blocker.inner.unblock_nvidia(*gpu.nvidia_minor())?
        }
        Ok(())
    }
}

fn gpu_node_ids(gpu: &Gpu) -> GpuResult<(u32, u32)> {
    let card_id = parse_node_id(gpu.card_node(), "card")?;
    let render_id = parse_node_id(gpu.render_node(), "renderD")?;
    Ok((card_id, render_id))
}

fn parse_node_id(node_path: &str, prefix: &str) -> GpuResult<u32> {
    let node = Path::new(node_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            IoError::new(
                ErrorKind::InvalidData,
                format!("Invalid DRM node path: {}", node_path),
            )
        })?;
    let id = node.strip_prefix(prefix).ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidData,
            format!("Unexpected DRM node name: {}", node),
        )
    })?;
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("Invalid DRM node id: {}", node),
        )
        .into());
    }
    Ok(id.parse()?)
}

fn map_gpu_error(err: impl std::fmt::Display) -> GpuError {
    GpuError::UnknownBlockState(err.to_string())
}
