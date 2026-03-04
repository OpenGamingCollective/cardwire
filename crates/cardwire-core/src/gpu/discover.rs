use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::iommu::Device;
use crate::gpu::models::Gpu;

pub fn read_gpu(pci_devices: &HashMap<String, Device>) -> io::Result<HashMap<usize, Gpu>> {
    let mut gpus: Vec<Gpu> = pci_devices
        .values()
        .filter(|device| device.class.as_str() == "0x030000")
        .map(|device| build_gpu(device))
        .collect::<io::Result<Vec<_>>>()?;

    // Default GPU gets ID 0, rest ordered by PCI address
    gpus.sort_by(|a, b| {
        b.default.cmp(&a.default).then(a.pci.cmp(&b.pci))
    });

    Ok(gpus
        .into_iter()
        .enumerate()
        .map(|(id, mut gpu)| { gpu.id = id as u32; (id, gpu) })
        .collect())
}

fn build_gpu(device: &Device) -> io::Result<Gpu> {
    let boot_vga_path = Path::new("/sys/bus/pci/devices")
        .join(&device.pci_address)
        .join("boot_vga");
    let is_default = fs::read_to_string(boot_vga_path)?.trim() == "1";

    Ok(Gpu {
        id: 0, // reassigned after sorting
        name: device.device_name.clone(),
        pci: device.pci_address.clone(),
        render: drm_node_path(&device.pci_address, "render")?,
        card: drm_node_path(&device.pci_address, "card")?,
        default: is_default,
    })
}

fn drm_node_path(pci_address: &str, node_kind: &str) -> io::Result<String> {
    let by_path = format!("/dev/dri/by-path/pci-{pci_address}-{node_kind}");
    Ok(fs::canonicalize(by_path)?
        .to_string_lossy()
        .into_owned())
}

