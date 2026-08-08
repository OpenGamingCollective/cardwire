//! GPU vendor model lookup from /proc and libdrm data files.

use std::{fs, path::Path};

pub fn nvidia_get_minor(pci_address: &str) -> Option<u32> {
    let nvidia_driver_proc = Path::new("/proc/driver/nvidia/gpus/")
        .join(pci_address)
        .join("information");
    let information = fs::read_to_string(nvidia_driver_proc).ok()?;
    information
        .lines()
        .find(|line| line.starts_with("Device Minor:"))?
        .split_once(':')?
        .1
        .trim()
        .parse::<u32>()
        .ok()
}

/// find the nvidia model using the device information file
pub fn nvidia_get_device_model(pci_address: &str) -> Option<String> {
    let nvidia_driver_proc = Path::new("/proc/driver/nvidia/gpus/")
        .join(pci_address)
        .join("information");
    let information = fs::read_to_string(nvidia_driver_proc).ok()?;
    let model = information
        .lines()
        .find(|line| line.starts_with("Model:"))?
        .split_once(':')?
        .1
        .trim()
        .to_string();
    match !model.is_empty() {
        true => Some(model),
        false => None,
    }
}

/// Find the amd model using amdgpu.ids, require the device id and the revision for precise matching
pub fn amd_get_device_model(device_id: &str, pci: &str) -> Option<String> {
    let path = "/usr/share/libdrm/amdgpu.ids";
    let device_id = device_id.to_string().replace("0x", "").to_ascii_uppercase();

    let revision = fs::read_to_string(format!("/sys/bus/pci/devices/{}/revision", pci))
        .ok()?
        .trim()
        .replace("0x", "")
        .to_ascii_uppercase();

    let content = fs::read_to_string(path).ok()?;

    for line in content.lines() {
        if line.starts_with('#') {
            continue;
        }

        let mut parts = line.split('\t');
        let Some(id) = parts.next() else {
            continue;
        };
        let Some(rev) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };

        if id.trim_end_matches(',') == device_id && rev.trim_end_matches(',') == revision {
            return Some(name.to_string());
        }
    }

    None
}
