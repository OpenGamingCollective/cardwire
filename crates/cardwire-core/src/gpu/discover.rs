use crate::{gpu::models::Gpu, pci::PciDevice};
use log::{info, warn};
use std::{
    collections::HashMap, fs, io::{self}, path::Path
};

pub fn read_gpu(pci_devices: &HashMap<String, PciDevice>) -> io::Result<HashMap<usize, Gpu>> {
    let gpus: Vec<Gpu> = pci_devices
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
        default: None,
        nvidia,
        nvidia_minor,
    })
}

fn drm_node_path(pci_address: &str, node_kind: &str) -> io::Result<u32> {
    let mut node_kind: String = node_kind.to_string();
    let by_path = format!("/dev/dri/by-path/pci-{pci_address}-{node_kind}");
    let kind_path = fs::canonicalize(&by_path)?;
    let file_name = kind_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid device path"))?;
    if node_kind == "render" {
        node_kind = "renderD".to_string();
    }
    let kind_number = file_name.strip_prefix(&node_kind).unwrap_or_default();
    Ok(kind_number.parse::<u32>().unwrap_or(999))
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
/*
    Method from kwin
*/
pub fn check_default_drm_class(gpu_list: &mut HashMap<usize, Gpu>) -> io::Result<()> {
    let class_path = Path::new("/sys/class/drm");
    let mut drm_entries = Vec::new();
    if class_path.exists() {
        for entry in fs::read_dir(class_path)? {
            let entry = entry?;
            drm_entries.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    #[derive(Default)]
    struct GpuStats {
        internal_displays: usize,
        desktop_displays: usize,
        total_displays: usize,
    }

    let mut stats: HashMap<usize, GpuStats> = HashMap::new();

    for (id, gpu) in &mut *gpu_list {
        let mut stat = GpuStats::default();
        let prefix = format!("card{}-", gpu.card);
        for name in &drm_entries {
            if let Some(drm) = name.strip_prefix(&prefix) {
                let status_path = class_path.join(name).join("status");
                if let Ok(status) = fs::read_to_string(&status_path) {
                    if status.trim() != "connected" {
                        continue;
                    }
                } else {
                    continue;
                }
                stat.total_displays += 1;

                if drm.starts_with("eDP") {
                    stat.internal_displays += 1;
                } else {
                    stat.desktop_displays += 1;
                }
            }
        }

        info!(
            "gpu {} id: {} internal: {}, desktop: {}, total: {}",
            gpu.name, id, stat.internal_displays, stat.desktop_displays, stat.total_displays
        );

        stats.insert(*id, stat);
    }

    let default = stats
        .iter()
        .max_by_key(|&(_, stats)| {
            (
                stats.internal_displays,
                stats.desktop_displays,
                stats.total_displays,
            )
        })
        .unzip();

    for gpu in gpu_list.values_mut() {
        if gpu.id == *default.0.unwrap() as u32 {
            gpu.default = Some(true);
        }
    }

    // Default GPU gets ID 0, rest ordered by PCI address
    let mut gpus: Vec<Gpu> = gpu_list.drain().map(|(_, gpu)| gpu).collect();
    gpus.sort_by(|a, b| b.default.cmp(&a.default).then(a.pci.cmp(&b.pci)));
    *gpu_list = gpus
        .into_iter()
        .enumerate()
        .map(|(id, mut gpu)| {
            gpu.id = id as u32;
            (id, gpu)
        })
        .collect();

    Ok(())
}
