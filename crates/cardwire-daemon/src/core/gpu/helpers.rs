//! Read a pci list and return a list of gpu
use crate::core::gpu::models::GpuDevice;
use log::{info, warn};
use std::{
    collections::{BTreeMap, HashMap}, fs, io, path::Path, time::Duration
};
use udev::{Device, Enumerator};

/// Return whether a DRM card currently owns a connected physical external display.
///
/// Connector ownership is encoded in sysfs names such as `card1-HDMI-A-1`. Internal panels and
/// virtual connectors are excluded so only physical external outputs keep the card available.
pub fn external_display_connected(card: u32) -> io::Result<bool> {
    // These connector types are internal panels or do not represent a physical display output.
    const NON_EXTERNAL: &[&str] = &[
        "eDP-",
        "LVDS-",
        "DSI-",
        "DPI-",
        "SPI-",
        "Virtual-",
        "Unknown-",
        "Writeback-",
    ];
    let card_prefix = format!("card{card}-");
    // An unreadable status is not proof of a disconnect. Keep the first error while checking
    // whether another connector can still confirm that the card is in use.
    let mut status_error = None;

    for entry in fs::read_dir("/sys/class/drm")? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(connector) = name.strip_prefix(&card_prefix) else {
            continue;
        };
        if connector.is_empty()
            || NON_EXTERNAL
                .iter()
                .any(|prefix| connector.starts_with(prefix))
        {
            continue;
        }

        match fs::read_to_string(entry.path().join("status")) {
            // A confirmed connection takes precedence over errors from other connectors.
            Ok(status) if status.trim() == "connected" => return Ok(true),
            Ok(_) => {}
            Err(err) => {
                status_error.get_or_insert(err);
            }
        }
    }

    // Fail safely instead of allowing incomplete topology information to block a display GPU.
    match status_error {
        Some(err) => Err(err),
        None => Ok(false),
    }
}

/// Reads both the card and render node IDs (e.g., (1, 128)) for a given PCI address.
/// Retries until both DRM nodes are spawned by the kernel and initialized by udev
pub fn drm_node_ids(pci_address: &str) -> io::Result<(u32, u32)> {
    const MAX_RETRIES: u32 = 10;
    const RETRY_INTERVAL: Duration = Duration::from_millis(500);

    let pci_syspath = Path::new("/sys/bus/pci/devices").join(pci_address);

    for attempt in 1..=MAX_RETRIES {
        let mut card_id = None;
        let mut render_id = None;

        if let Ok(parent) = Device::from_syspath(&pci_syspath)
            && let Ok(mut enumerator) = Enumerator::new()
        {
            let _ = enumerator.match_parent(&parent);
            let _ = enumerator.match_subsystem("drm");

            if let Ok(devices) = enumerator.scan_devices() {
                for dev in devices {
                    // Skip if uninitialized
                    if !dev.is_initialized() {
                        continue;
                    }

                    let sysname = dev.sysname().to_string_lossy();

                    if let Some(num) = dev.sysnum() {
                        if sysname == format!("card{num}") {
                            card_id = Some(num as u32);
                        } else if sysname == format!("renderD{num}") {
                            render_id = Some(num as u32);
                        }
                    }
                }
            }
        }

        if let (Some(card), Some(render)) = (card_id, render_id) {
            info!(
                "Successfully resolved card{} and renderD{} for PCI {}",
                card, render, pci_address
            );
            return Ok((card, render));
        }

        if attempt < MAX_RETRIES {
            warn!(
                "DRM nodes (card/render) for PCI {} not fully ready, attempt {}/{MAX_RETRIES}, retrying in 500ms...",
                pci_address, attempt
            );
            std::thread::sleep(RETRY_INTERVAL);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "Failed to find both initialized card and render DRM nodes for PCI {}",
            pci_address
        ),
    ))
}

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

/// Method from kwin
pub fn check_default_drm_class(gpu_list: &mut BTreeMap<usize, GpuDevice>) -> io::Result<()> {
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
        let prefix = format!("card{}-", gpu.card());
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
            gpu.name(),
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

    for (id, gpu) in &mut *gpu_list {
        if let Some(default_id) = default.0 {
            if id == default_id {
                gpu.set_default(Some(true));
            } else {
                gpu.set_default(Some(false));
            }
        }
    }

    // Default GPU gets ID 0, rest ordered by PCI address
    let mut gpus: Vec<GpuDevice> = std::mem::take(gpu_list).into_values().collect();
    gpus.sort_by(|a, b| {
        b.default()
            .cmp(&a.default())
            .then(a.pci.pci_address().cmp(b.pci.pci_address()))
    });
    *gpu_list = gpus.into_iter().enumerate().collect();

    Ok(())
}
