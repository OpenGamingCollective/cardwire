use crate::{gpu::models::Gpu, pci::PciDevice};
use log::{debug, warn};
use std::{collections::HashMap, fs, io, path::Path};

pub fn read_gpu(pci_devices: &HashMap<String, PciDevice>) -> io::Result<HashMap<usize, Gpu>> {
    let mut gpus: Vec<Gpu> = pci_devices
        .values()
        .filter(|device| {
            device.class.as_deref() == Some("0x030000") || // VGA compatible controller
            device.class.as_deref() == Some("0x030100") || // XGA compatible controller
            device.class.as_deref() == Some("0x030200") || // 3D Controller
            device.class.as_deref() == Some("0x038000") // Display controller
        })
        .filter_map(|device| match build_gpu(device) {
            Ok(gpu) => Some(gpu),
            Err(e) => {
                warn!("Failed to build GPU for PCI {}: {}", device.pci_address, e);
                None
            }
        })
        .collect();

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

fn build_gpu(device: &PciDevice) -> io::Result<Gpu> {
    let nvidia: bool = device.vendor_id.as_deref() == Some("0x10de");
    let nvidia_minor: u32 = if nvidia {
        nvidia_get_minor(&device.pci_address).unwrap_or(99)
    } else {
        99
    };

    Ok(Gpu {
        id: 0, // reassigned after sorting
        name: device
            .device_name
            .clone()
            .unwrap_or_else(|| "Unknown Device".to_string()),
        pci: device.pci_address.clone(),
        render: drm_node_path(&device.pci_address, "render")?,
        card: drm_node_path(&device.pci_address, "card")?,
        default: check_default(&device.pci_address)?,
        nvidia,
        nvidia_minor,
    })
}

fn drm_node_path(pci_address: &str, node_kind: &str) -> io::Result<String> {
    let by_path = format!("/dev/dri/by-path/pci-{pci_address}-{node_kind}");
    match fs::canonicalize(&by_path) {
        Ok(path) => Ok(path.to_string_lossy().into_owned()),
        Err(err) => {
            warn!(
                "Could not find DRM node {} for PCI {}: {}",
                by_path, pci_address, err
            );
            Err(err)
        }
    }
}
fn nvidia_get_minor(pci_address: &str) -> Option<u32> {
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
fn check_default(pci_address: &str) -> io::Result<bool> {
    let fb0_path = "/sys/class/graphics/fb0";
    match fs::canonicalize(fb0_path) {
        Ok(content) => Ok(content.to_string_lossy().contains(pci_address)),
        Err(_) => {
            debug!("check_default: {} isn't the default gpu", pci_address);
            Ok(false)
        }
    }
}
