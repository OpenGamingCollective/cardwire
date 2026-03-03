use crate::gpu::models::Gpu;
use cardwire_ebpf::EbpfBlocker;
use std::io::{Error as IoError, ErrorKind};
use std::path::Path;

pub fn is_gpu_blocked(blocker: &mut EbpfBlocker, gpu: &Gpu) -> Result<bool, Box<dyn std::error::Error>> {
    let (card_id, render_id) = gpu_node_ids(gpu)?;
    Ok(
        blocker.is_pci_blocked(gpu.pci_address())?
            && blocker.is_id_blocked(card_id)?
            && blocker.is_id_blocked(render_id)?,
    )
}


pub fn block_gpu(blocker: &mut EbpfBlocker, gpu: &Gpu, block: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (card_id, render_id) = gpu_node_ids(gpu)?;

    if block {
        blocker.block_id(card_id)?;
        blocker.block_id(render_id)?;
        blocker.block_pci(gpu.pci_address())
    } else {
        blocker.unblock_id(card_id)?;
        blocker.unblock_id(render_id)?;
        blocker.unblock_pci(gpu.pci_address())
    }
}

fn gpu_node_ids(gpu: &Gpu) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    let card_id = parse_node_id(gpu.card_node(), "card")?;
    let render_id = parse_node_id(gpu.render_node(), "renderD")?;
    Ok((card_id, render_id))
}

fn parse_node_id(node_path: &str, prefix: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let node = Path::new(node_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, format!("Invalid DRM node path: {}", node_path)))?;
    let id = node
        .strip_prefix(prefix)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, format!("Unexpected DRM node name: {}", node)))?;
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(IoError::new(ErrorKind::InvalidData, format!("Invalid DRM node id: {}", node)).into());
    }
    Ok(id.parse()?)
}
