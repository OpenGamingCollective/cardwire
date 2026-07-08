//! Used to get inodes for specific files
use std::{
    collections::BTreeMap, fs::{self}, os::unix::fs::MetadataExt
};

use anyhow::Result;
use log::warn;

use crate::core::pci::PciDevice;

// shouldn't be necessary anymore
const _BLOCKED_PCI_FILES: &[&str] = &[
    "config",
    "current_link_speed",
    "max_link_speed",
    "max_link_width",
    "current_link_width",
];

/// Files that get blocked when the NVIDIA block is on
const BLOCKED_NVIDIA_FILES: &[&str] = &[
    "libGLX_nvidia.so.0",
    "nvidia_icd.json",
    "nvidia_icd.x86_64.json",
    "nvidiactl",
];

pub fn render_to_inode(render: u32) -> Result<u64> {
    let render_path = format!("/dev/dri/renderD{}", render);
    let metadata = fs::metadata(&render_path).map_err(|e| {
        warn!("failed to get inode for {}: {}", render_path, e);
        e
    })?;
    let inode = metadata.ino();

    Ok(inode)
}

pub fn card_to_inode(card: u32) -> Result<u64> {
    let card_path = format!("/dev/dri/card{}", card);
    let metadata = fs::metadata(&card_path).map_err(|e| {
        warn!("failed to get inode for {}: {}", card_path, e);
        e
    })?;
    let inode = metadata.ino();

    Ok(inode)
}

// Here return a list of inode that contain the pci card, the audio card and their parents
pub fn pci_to_inode(
    pci: String,
    parent_pci: &Option<String>,
    pci_list: &BTreeMap<String, PciDevice>,
) -> Result<Vec<u64>> {
    let mut inodes: Vec<u64> = Vec::new();

    // quick function return Some(inode) if successfull or return None is fail
    let get_pci_inode = |pci: &str| {
        let pci_path = format!("/sys/bus/pci/devices/{}", pci);
        if let Ok(metadata) = fs::metadata(&pci_path) {
            return Some(metadata.ino());
        }
        None
    };

    // first we push the pci inode
    match get_pci_inode(&pci) {
        Some(inode) => inodes.push(inode),
        None => {
            warn!("failed to get inode for pci: {}", pci);
        }
    }
    // Also push the audio card inode
    match get_pci_inode(&pci.replace(".0", ".1")) {
        Some(inode) => inodes.push(inode),
        None => {
            warn!("failed to get inode for pci: {}", &pci.replace(".0", ".1"));
        }
    }

    let mut current_parent: Option<String> = parent_pci.clone();
    while let Some(parent_pci) = current_parent {
        if let Some(pci_device) = pci_list.get(&parent_pci) {
            current_parent = pci_device.parent_pci().clone();
            match get_pci_inode(pci_device.pci_address()) {
                Some(inode) => inodes.push(inode),
                None => {
                    warn!("failed to get inode for pci: {}", pci_device.pci_address());
                    continue;
                }
            }
        } else {
            warn!("expected parent pci {} not found in pci_list", parent_pci);
            break;
        }
    }

    Ok(inodes)
}

/// Used to verify the block status of a single pci
pub fn single_pci_to_inode(pci: &str) -> Result<u64> {
    let pci_path = format!("/sys/bus/pci/devices/{}", pci);
    let metadata = fs::metadata(&pci_path).map_err(|e| {
        warn!("failed to get inode for {}: {}", pci_path, e);
        e
    })?;
    let inode = metadata.ino();

    Ok(inode)
}

pub fn nvidia_to_inode(nvidia_minor: u32) -> Result<u64> {
    let nvidia_path = format!("/dev/nvidia{}", nvidia_minor);
    let metadata = fs::metadata(&nvidia_path).map_err(|e| {
        warn!("failed to get inode for {}: {}", nvidia_path, e);
        e
    })?;
    let inode = metadata.ino();

    Ok(inode)
}
