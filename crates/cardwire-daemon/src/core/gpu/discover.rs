//! Read a pci list and return a list of gpu
use crate::{
    core::{
        gpu::models::{GpuDevice, GpuVendor}, pci::PciDevice
    }, interface::GpuInterface
};
use log::{info, warn};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap}, fs, io, path::Path
};

const DRM_CLASS_PATH: &str = "/sys/class/drm";

#[derive(Default)]
struct GpuStats {
    internal_displays: usize,
    desktop_displays: usize,
    total_displays: usize,
    connected_displays: usize,
    connected_internal: usize,
    connected_desktop: usize,
}

fn default_gpu_rank(stats: &GpuStats) -> (usize, usize, usize, usize, usize) {
    (
        stats.connected_internal,
        stats.connected_desktop,
        stats.internal_displays,
        stats.desktop_displays,
        stats.total_displays,
    )
}

/// read a map of pci devices and return a map of gpu devices
pub fn read_gpu(
    pci_devices: &BTreeMap<String, PciDevice>,
) -> io::Result<BTreeMap<usize, GpuDevice>> {
    let mut gpus: BTreeMap<usize, GpuDevice> = BTreeMap::new();
    // We use i as the key to have some sort of sorted list, this number will get re-assigned later
    // when searching for the default gpu
    let mut i = 0;
    // take pci_devices map and filter to only keep display controller class
    // 03 means it's a display controller, see <https://admin.pci-ids.ucw.cz/read/PD/>
    for device in pci_devices.values().filter(|dev| {
        dev.class()
            .as_ref()
            .is_some_and(|class| class.starts_with("0x03"))
    }) {
        gpus.insert(i, build_gpu(device)?);
        if gpus.contains_key(&i) {
            i += 1;
        }
    }
    Ok(gpus)
}

/// Return the DRM card numbers that currently have a connected external display.
///
/// Connector ownership is encoded by sysfs in names such as `card1-HDMI-A-1` and
/// `card0-DP-2`, so this avoids making assumptions about PCI order or connector names.
pub fn connected_external_drm_cards() -> io::Result<BTreeSet<u32>> {
    connected_external_drm_cards_at(Path::new(DRM_CLASS_PATH))
}

fn connected_external_drm_cards_at(class_path: &Path) -> io::Result<BTreeSet<u32>> {
    let mut connected_cards = BTreeSet::new();

    for entry in fs::read_dir(class_path)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some((card, connector)) = parse_drm_connector_name(&name) else {
            continue;
        };
        if !is_external_connector(connector) {
            continue;
        }

        let status = fs::read_to_string(entry.path().join("status"));
        if status.is_ok_and(|status| status.trim() == "connected") {
            connected_cards.insert(card);
        }
    }

    Ok(connected_cards)
}

fn parse_drm_connector_name(name: &str) -> Option<(u32, &str)> {
    let name = name.strip_prefix("card")?;
    let (card, connector) = name.split_once('-')?;
    Some((card.parse().ok()?, connector))
}

fn is_external_connector(connector: &str) -> bool {
    !is_internal_connector(connector) && !connector.starts_with("Writeback")
}

fn is_internal_connector(connector: &str) -> bool {
    connector.starts_with("eDP") || connector.starts_with("LVDS") || connector.starts_with("DSI")
}

/// Take a pci device and build a gpu device from it
fn build_gpu(device: &PciDevice) -> io::Result<GpuDevice> {
    let gpu_vendor = match device.vendor_id() {
        Some(vendor_id) => GpuVendor::from(vendor_id.as_str()),
        // Default to other
        None => GpuVendor::default(),
    };
    // nvidia_minor is used in /dev/nvidia<i>, where i is the minor number eg: nvidia0
    // None if not a nvidia device
    let nvidia_minor: Option<u32> = match gpu_vendor {
        GpuVendor::Nvidia => nvidia_get_minor(device.pci_address()),
        _ => None,
    };

    let device_name = match gpu_vendor {
        GpuVendor::Nvidia => nvidia_get_device_model(device.pci_address()),
        GpuVendor::Amd => match device.device_id() {
            Some(id) => amd_get_device_model(id, device.pci_address()),
            None => None,
        },
        _ => None,
    }
    // If none, just use the hwdata name or a placeholder
    .unwrap_or_else(|| {
        warn!("Couldn't get device_name, falling back to hwdata");
        device.device_name().clone().unwrap_or_else(|| {
            warn!("Couldn't get name using hwdata, falling back to default");
            "Unknown Device".to_string()
        })
    });

    Ok(GpuDevice::new(
        device_name,
        device.clone(),
        // propagate err on purpose if the drm nodes return errors, if there is no nodes we want to
        // skip this gpu
        drm_node_path(device.pci_address(), "render")?,
        drm_node_path(device.pci_address(), "card")?,
        None,
        gpu_vendor,
        nvidia_minor,
    ))
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

/// find the nvidai model using the device information file
fn nvidia_get_device_model(pci_address: &str) -> Option<String> {
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
fn amd_get_device_model(device_id: &str, pci: &str) -> Option<String> {
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

/// Method from kwin
pub fn check_default_drm_class(gpu_list: &mut BTreeMap<usize, GpuInterface>) -> io::Result<()> {
    // skip if empty
    if gpu_list.is_empty() {
        return Ok(());
    }
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
    let mut stats: HashMap<usize, GpuStats> = HashMap::new();

    for (id, gpu) in &mut *gpu_list {
        let mut stat = GpuStats::default();
        let prefix = format!("card{}-", gpu.device.card());
        for name in &drm_entries {
            if let Some(drm) = name.strip_prefix(&prefix) {
                if !is_internal_connector(drm) && !is_external_connector(drm) {
                    continue;
                }
                let status_path = class_path.join(name).join("status");
                //
                if let Ok(status) = fs::read_to_string(&status_path) {
                    stat.total_displays += 1;
                    let is_connected = status.trim() == "connected";
                    if is_connected {
                        stat.connected_displays += 1;
                    }
                    if is_internal_connector(drm) {
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
            gpu.device.name(),
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
        .max_by_key(|&(&id, stats)| (default_gpu_rank(stats), std::cmp::Reverse(id)))
        .unzip();

    for (id, gpu) in &mut *gpu_list {
        if let Some(default_id) = default.0 {
            if id == default_id {
                gpu.device.set_default(Some(true));
            } else {
                gpu.device.set_default(Some(false));
            }
        }
    }

    // Default GPU gets ID 0, rest ordered by PCI address
    let mut gpus: Vec<GpuInterface> = std::mem::take(gpu_list).into_values().collect();
    gpus.sort_by(|a, b| {
        b.device
            .default()
            .cmp(&a.device.default())
            .then(a.device.pci.pci_address().cmp(b.device.pci.pci_address()))
    });
    *gpu_list = gpus.into_iter().enumerate().collect();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf, sync::atomic::{AtomicU64, Ordering}
    };

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDrmRoot(PathBuf);

    impl TempDrmRoot {
        fn new() -> Self {
            let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("cardwire-drm-test-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn connector(&self, name: &str, status: &str) {
            let path = self.0.join(name);
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("status"), status).unwrap();
        }
    }

    impl Drop for TempDrmRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parse_connector_names_keeps_card_ownership() {
        assert_eq!(
            parse_drm_connector_name("card1-HDMI-A-1"),
            Some((1, "HDMI-A-1"))
        );
        assert_eq!(parse_drm_connector_name("card12-DP-3"), Some((12, "DP-3")));
        assert_eq!(parse_drm_connector_name("renderD128"), None);
        assert_eq!(parse_drm_connector_name("card1"), None);
    }

    #[test]
    fn external_connector_classification_ignores_panels_and_writeback() {
        for connector in ["eDP-1", "LVDS-1", "DSI-1", "Writeback-1"] {
            assert!(!is_external_connector(connector));
        }
        for connector in ["HDMI-A-1", "DP-1", "DVI-D-1"] {
            assert!(is_external_connector(connector));
        }
        for connector in ["eDP-1", "LVDS-1", "DSI-1"] {
            assert!(is_internal_connector(connector));
        }
        assert!(!is_internal_connector("Writeback-1"));
    }

    #[test]
    fn connected_external_cards_are_discovered_from_status() {
        let root = TempDrmRoot::new();
        root.connector("card0-eDP-1", "connected\n");
        root.connector("card0-DP-1", "connected\n");
        root.connector("card1-HDMI-A-1", "connected\n");
        root.connector("card2-DP-2", "disconnected\n");
        root.connector("card3-Writeback-1", "connected\n");
        fs::create_dir_all(root.0.join("card4-DP-3")).unwrap();

        assert_eq!(
            connected_external_drm_cards_at(&root.0).unwrap(),
            BTreeSet::from([0, 1])
        );
    }

    #[test]
    fn connected_external_display_outranks_disconnected_internal_connector() {
        let integrated = GpuStats {
            internal_displays: 1,
            ..GpuStats::default()
        };
        let discrete = GpuStats {
            desktop_displays: 1,
            connected_displays: 1,
            connected_desktop: 1,
            ..GpuStats::default()
        };

        assert!(default_gpu_rank(&discrete) > default_gpu_rank(&integrated));
    }
}
