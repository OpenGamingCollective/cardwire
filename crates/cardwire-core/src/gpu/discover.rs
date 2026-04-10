use std::collections::HashMap;
use std::fs;
use std::io;

use crate::gpu::models::Gpu;
use crate::iommu::Device;

pub fn read_gpu(pci_devices: &HashMap<String, Device>) -> io::Result<HashMap<usize, Gpu>> {
    let mut gpus: Vec<Gpu> = pci_devices
        .values()
        .filter(|device| {
            device.class.as_str() == "0x030000" || // VGA compatible controller
            device.class.as_str() == "0x038000"
        }) // Display controller
        .map(|device| build_gpu(device))
        .collect::<io::Result<Vec<_>>>()?;

    // Default GPU gets ID 0, rest ordered by PCI address
    gpus.sort_by(|a, b| b.default.cmp(&a.default).then(a.pci.cmp(&b.pci)));

    Ok(gpus
        .into_iter()
        .enumerate()
        .map(|(id, mut gpu)| {
            gpu.id = id as u32;
            (id, gpu)
        })
        .collect())
}

fn build_gpu(device: &Device) -> io::Result<Gpu> {
    let nvidia: bool = &device.vendor_id == "0x10de";
    let nvidia_minor: u32 = if nvidia {
        nvidia_get_minor(&device.pci_address).unwrap_or(99)
    } else {
        99
    };

    Ok(Gpu {
        id: 0, // reassigned after sorting
        name: device.device_name.clone(),
        pci: device.pci_address.clone(),
        render: drm_node_path(&device.pci_address, "render")?,
        card: drm_node_path(&device.pci_address, "card")?,
        default: is_default,
        nvidia: nvidia,
        nvidia_minor: nvidia_minor,
        default: check_default(&device.pci_address)?,
    })
}

fn drm_node_path(pci_address: &str, node_kind: &str) -> io::Result<String> {
    let by_path = format!("/dev/dri/by-path/pci-{pci_address}-{node_kind}");
    Ok(fs::canonicalize(by_path)?.to_string_lossy().into_owned())
}
fn nvidia_get_minor(pci_address: &str) -> Option<u32> {
    let nvidia_driver_proc = Path::new("/proc/driver/nvidia/gpus/").join(pci_address).join("information");
    let information =
        fs::read_to_string(nvidia_driver_proc).expect("Couldn't read nvidia information");
    information
        .lines()
        .find(|line| line.starts_with("Device Minor:"))?
        .split_once(':')?
        .1
        .trim()
        .parse::<u32>()
        .ok()
fn check_default(pci_address: &str) -> io::Result<bool> {
    let fb0_path = format!("/sys/class/graphics/fb0");
    match fs::canonicalize(fb0_path) {
        Ok(content) => Ok(content.to_string_lossy().contains(pci_address)),
        Err(_) => Ok(false),
    }
}
