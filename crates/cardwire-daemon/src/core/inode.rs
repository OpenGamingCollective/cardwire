//! Used to get inodes for specific files
use std::{
    collections::BTreeMap, fs::{self}, os::unix::fs::MetadataExt, path::Path
};

use anyhow::Result;
use log::{error, warn};

use crate::core::pci::PciDevice;

pub fn get_inodes(
    render: u32,
    card: u32,
    pci: &str,
    parent_pci: &Option<String>,
    pci_list: &BTreeMap<String, PciDevice>,
    nvidia_minor: Option<u32>,
) -> Result<Vec<u64>> {
    let mut total_inodes: Vec<u64> = Vec::new();

    match card_to_inode(card) {
        Ok(inode_res) => total_inodes.push(inode_res),
        Err(err) => {
            error!("failed to get inode for card{}: {}", card, err);
            return Err(err);
        }
    };
    match render_to_inode(render) {
        Ok(inode_res) => total_inodes.push(inode_res),
        Err(err) => {
            error!("failed to get inode for renderD{}: {}", render, err);
            return Err(err);
        }
    };
    match pci_to_inode(pci.to_string(), parent_pci, pci_list) {
        Ok(mut inodes_res) => {
            total_inodes.append(&mut inodes_res);
        }
        Err(err) => {
            error!("failed to get inode for pci {}: {}", pci, err);
            return Err(err);
        }
    };
    // Block files in /sys/class/drm
    match sys_drm_inodes(render, card) {
        Ok(mut inodes_res) => {
            total_inodes.append(&mut inodes_res);
        }
        Err(err) => {
            error!("failed to get inode for drm {}: {}", pci, err);
            return Err(err);
        }
    };
    // Block files in /sys/class/hwmon and pci sysfs
    match sys_hwmon(pci) {
        Ok(mut inodes_res) => {
            total_inodes.append(&mut inodes_res);
        }
        Err(err) => {
            // ignored because VMs gpu do not have hwmon
            error!("(ignoring) failed to get inode for hwmon {}: {}", pci, err);
        }
    };

    if let Some(minor) = nvidia_minor {
        match nvidia_to_inode(minor) {
            Ok(inode) => total_inodes.push(inode),
            Err(err) => {
                error!("failed to get inode for nvidia{}: {}", minor, err);
                return Err(err);
            }
        };
        match backlight_to_inode(minor) {
            Ok(inode) => total_inodes.push(inode),
            Err(err) => {
                error!(
                    "(ignoring) failed to get inode for backlight nvidia_{}: {}",
                    minor, err
                );
            }
        };
    }

    Ok(total_inodes)
}

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

    // quick function that push the inodes into the vec
    let push_pci_inode = |pci: &str, inodes: &mut Vec<u64>| {
        // First get the link ino
        let pci_path = format!("/sys/bus/pci/devices/{}", pci);
        if let Ok(metadata) = fs::metadata(&pci_path) {
            inodes.push(metadata.ino());
        }

        // Now without following link
        let pci_path = format!("/sys/bus/pci/devices/{}", pci);
        if let Ok(metadata) = fs::symlink_metadata(&pci_path) {
            inodes.push(metadata.ino());
        }
    };

    // first we push the pci inode
    push_pci_inode(&pci, &mut inodes);
    // Also push the audio card inode
    push_pci_inode(&pci.replace(".0", ".1"), &mut inodes);

    let mut current_parent: Option<String> = parent_pci.clone();
    while let Some(parent_pci) = current_parent {
        if let Some(pci_device) = pci_list.get(&parent_pci) {
            current_parent = pci_device.parent_pci().clone();
            push_pci_inode(pci_device.pci_address(), &mut inodes);
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

/// The only gpu vendor that need it's backlight to be blocked is nvidia
pub fn backlight_to_inode(nvidia_minor: u32) -> Result<u64> {
    let nvidia_path = format!("/sys/class/backlight/nvidia_{}", nvidia_minor);
    let metadata = fs::metadata(&nvidia_path).map_err(|e| {
        warn!("failed to get inode for {}: {}", nvidia_path, e);
        e
    })?;
    let inode = metadata.ino();

    Ok(inode)
}

pub fn exp_nvidia_inodes() -> Result<Vec<u64>> {
    let mut inodes: Vec<u64> = Vec::new();

    // Get nvidiactl inode
    let nvidiactl = "/dev/nvidiactl";
    if let Ok(metadata) = fs::metadata(nvidiactl) {
        inodes.push(metadata.ino());
    }

    // Now try to find the vulkan file
    // This is for normal distros
    /// Files that get blocked when the NVIDIA block is on
    const VULKAN_PATHS: &[&str] = &[
        // NixOS
        "/run/opengl-driver/share/vulkan/icd.d/",
        "/run/opengl-driver-32/share/vulkan/icd.d/",
        // Standard Linux
        "/etc/vulkan/icd.d/",
        "/usr/share/vulkan/icd.d/",
    ];

    for path in VULKAN_PATHS {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };

        for entry in entries.flatten() {
            let name = entry.file_name();

            if (name == "nvidia_icd.json"
                || name == "nvidia_icd.x86_64.json"
                || name == "nvidia_icd.i686.json")
                && let Ok(metadata) = fs::metadata(entry.path())
                && metadata.is_file()
            {
                inodes.push(metadata.ino());
            }
        }
    }

    Ok(inodes)
}

pub fn sys_drm_inodes(render: u32, card: u32) -> Result<Vec<u64>> {
    let mut inodes = Vec::new();
    let sys_path = Path::new("/sys/class/drm");

    let card = format!("card{}", card);
    let render = format!("renderD{}", render);
    for entry in fs::read_dir(sys_path)? {
        let entry = entry?;
        if let Ok(entry_name) = entry.file_name().into_string()
            && (entry_name.contains(&card) || entry_name.contains(&render))
        {
            // we matched with the blocked device, get the inodes without following the link
            let inode_res = fs::symlink_metadata(entry.path());
            if let Ok(meta) = inode_res {
                inodes.push(meta.ino());
            }
        }
    }

    Ok(inodes)
}

pub fn sys_hwmon(pci: &str) -> Result<Vec<u64>> {
    let mut inodes = Vec::new();
    let sysfs_pci_path = format!("/sys/bus/pci/devices/{}/hwmon", pci);
    let sysfs_pci_path = Path::new(&sysfs_pci_path);

    for entry in fs::read_dir(sysfs_pci_path)? {
        let entry = entry?;
        // First add hwmon from the sysfs pci folder
        if let Ok(meta) = fs::metadata(entry.path()) {
            inodes.push(meta.ino());
        }
        // Then add from /sys/class/hwmon
        if let Ok(hwmon_entry) = entry.file_name().into_string() {
            let hwmon_path = format!("/sys/class/hwmon/{}", hwmon_entry);
            let hwmon_path = Path::new(&hwmon_path);
            // skip if folder doesnt exist
            if !hwmon_path.exists() {
                continue;
            }
            if let Ok(meta) = fs::metadata(hwmon_path) {
                inodes.push(meta.ino());
            }
            if let Ok(meta) = fs::symlink_metadata(hwmon_path) {
                inodes.push(meta.ino());
            }
        }
    }

    Ok(inodes)
}
