use crate::{gpu::models::Gpu, pci::PciDevice};
use log::{info, warn};
use std::{
    collections::{BTreeMap, HashMap}, fs, io, path::Path
};

pub fn read_gpu(pci_devices: &BTreeMap<String, PciDevice>) -> io::Result<BTreeMap<usize, Gpu>> {
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
    let nvidia_minor: Option<u32> = if nvidia {
        nvidia_get_minor(&device.pci_address)
    } else {
        None
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
        blocked: None,
        nvidia,
        nvidia_minor,
    })
}
/// Try to read from sysfs first, then fallback to udev /dev/dri
/// with a sleep at each attempt so the system has time to spawn the drm nodes
/// May block for up to ~5s per path (10 retries × 500ms)
fn drm_node_path(pci_address: &str, node_kind: &str) -> io::Result<u32> {
    const MAX_RETRIES: u32 = 10;
    const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

    let kind_prefix = match node_kind {
        "render" => "renderD",
        other => other,
    };
    let sysfs_drm_path = format!("/sys/bus/pci/devices/{}/drm", pci_address);
    let udev_drm_path = format!("/dev/dri/by-path/pci-{pci_address}-{node_kind}");
    let mut last_err: Option<io::Error> = None;

    for attempt in 1..=MAX_RETRIES {
        if let Ok(entries) = fs::read_dir(&sysfs_drm_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                let kind_number = name.strip_prefix(kind_prefix).unwrap_or_default();
                let is_match = (kind_prefix == "renderD" && name.starts_with("renderD"))
                    || (kind_prefix == "card" && name.starts_with("card") && !name.contains('-'));
                if is_match {
                    info!(
                        "Successfully read {}{} from sysfs for {}",
                        kind_prefix, kind_number, pci_address
                    );
                    return kind_number.parse::<u32>().map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Failed to parse DRM node number from '{name}'"),
                        )
                    });
                }
            }
            break;
        }
        warn!(
            "Could not find drm {} for pci {}, attempt: {}/{MAX_RETRIES}, retrying in 500ms",
            kind_prefix, pci_address, attempt
        );
        std::thread::sleep(RETRY_INTERVAL);
    }
    warn!(
        "Could not read {} drm {} from sysfs, falling back to /dev/dri",
        pci_address, kind_prefix
    );
    for attempt in 1..=MAX_RETRIES {
        match fs::canonicalize(&udev_drm_path) {
            Ok(kind_path) => {
                let file_name =
                    kind_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "Invalid device path")
                        })?;
                let kind_number = file_name.strip_prefix(kind_prefix).unwrap_or_default();
                return kind_number.parse::<u32>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Failed to parse DRM node number from '{file_name}'"),
                    )
                });
            }
            Err(err) => {
                warn!(
                    "Could not find {} node for {}: {}, attempt: {}/{MAX_RETRIES}, retrying in 500ms",
                    kind_prefix, pci_address, err, attempt
                );
                last_err = Some(err);
                std::thread::sleep(RETRY_INTERVAL);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Node not found")))
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
/// Method from kwin
pub fn check_default_drm_class(gpu_list: &mut BTreeMap<usize, Gpu>) -> io::Result<()> {
    let class_path = Path::new("/sys/class/drm");
    let mut drm_entries = Vec::new();
    if class_path.exists() {
        match fs::read_dir(class_path) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    drm_entries.push(entry.file_name().to_string_lossy().into_owned());
                }
            }
            Err(err) => {
                warn!(
                    "Could not read /sys/class/drm: {}, skipping default detection",
                    err
                );
            }
        }
    } else {
        warn!("/sys/class/drm does not exist, skipping default detection");
    }
    #[derive(Default)]
    struct GpuStats {
        internal_displays: usize,
        desktop_displays: usize,
        total_displays: usize,
        connected_displays: usize,
        connected_internal: usize,
        connected_desktop: usize,
    }

    let mut stats: HashMap<usize, GpuStats> = HashMap::new();

    for (id, gpu) in &mut *gpu_list {
        let mut stat = GpuStats::default();
        let prefix = format!("card{}-", gpu.card);
        for name in &drm_entries {
            if let Some(drm) = name.strip_prefix(&prefix) {
                let status_path = class_path.join(name).join("status");
                //
                if let Ok(status) = fs::read_to_string(&status_path) {
                    stat.total_displays += 1;
                    let is_connected = status.trim() == "connected";
                    if is_connected {
                        stat.connected_displays += 1;
                    }
                    if drm.starts_with("eDP") {
                        stat.internal_displays += 1;
                        if is_connected {
                            stat.connected_internal += 1;
                        }
                    } else {
                        stat.desktop_displays += 1;
                        if is_connected {
                            stat.connected_desktop += 1;
                        }
                    }
                }
            }
        }

        info!(
            "gpu {} id: {} internal: {}, desktop: {}, connected: {}, total: {}, connected_internal: {}, connected_desktop: {}",
            gpu.name,
            id,
            stat.internal_displays,
            stat.desktop_displays,
            stat.connected_displays,
            stat.total_displays,
            stat.connected_internal,
            stat.connected_desktop
        );

        stats.insert(*id, stat);
    }

    let default = stats
        .iter()
        .max_by_key(|&(_, stats)| {
            (
                stats.connected_internal,
                stats.connected_desktop,
                stats.internal_displays,
                stats.desktop_displays,
                stats.total_displays,
            )
        })
        .unzip();

    for gpu in gpu_list.values_mut() {
        if gpu.id == *default.0.unwrap() as u32 {
            gpu.default = Some(true);
        } else {
            gpu.default = Some(false);
        }
    }

    // Default GPU gets ID 0, rest ordered by PCI address
    let mut gpus: Vec<Gpu> = std::mem::take(gpu_list).into_values().collect();
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
