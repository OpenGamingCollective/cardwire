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
            device.class.as_str() == "0x038000"})  // Display controller
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
    Ok(Gpu {
        id: 0, // reassigned after sorting
        name: device.device_name.clone(),
        pci: device.pci_address.clone(),
        render: drm_node_path(&device.pci_address, "render")?,
        card: drm_node_path(&device.pci_address, "card")?,
        default: check_default(&device.pci_address)?,
    })
}

fn drm_node_path(pci_address: &str, node_kind: &str) -> io::Result<String> {
    let by_path = format!("/dev/dri/by-path/pci-{pci_address}-{node_kind}");
    Ok(fs::canonicalize(by_path)?.to_string_lossy().into_owned())
}
fn check_default(pci_address: &str) -> io::Result<bool> {
    let fb0_path =format!("/sys/class/graphics/fb0");
    match fs::canonicalize(fb0_path) {
        Ok(content) => Ok(content.to_string_lossy().
            contains(pci_address)),
        Err(_) => Ok(false)
    }
}
