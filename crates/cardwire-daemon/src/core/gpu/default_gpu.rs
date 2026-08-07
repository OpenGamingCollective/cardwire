//! KDE KWin-derived heuristic for identifying the default/boot GPU.

use crate::core::gpu::models::GpuDevice;
use log::{info, warn};
use std::{
    collections::{BTreeMap, HashMap}, fs, io, path::Path
};

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
        if !gpu.is_available() {
            gpu.set_default(Some(false));
            continue;
        }

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

    // On equal display counts (e.g. one monitor connected per
    // GPU on a desktop), prefer the GPU with the lowest PCI address instead of relying on the
    // arbitrary iteration order of a HashMap.
    let mut gpu_ids: Vec<usize> = stats.keys().copied().collect();
    gpu_ids.sort_unstable();
    let default_id = gpu_ids.into_iter().max_by(|&a, &b| {
        let sa = &stats[&a];
        let sb = &stats[&b];
        (
            sa.connected_internal,
            sa.connected_desktop,
            sa.internal_displays,
            sa.desktop_displays,
            sa.total_displays,
        )
            .cmp(&(
                sb.connected_internal,
                sb.connected_desktop,
                sb.internal_displays,
                sb.desktop_displays,
                sb.total_displays,
            ))
            .then_with(|| {
                gpu_list[&b]
                    .pci
                    .pci_address()
                    .cmp(gpu_list[&a].pci.pci_address())
            })
    });

    for (id, gpu) in &mut *gpu_list {
        if !gpu.is_available() {
            gpu.set_default(Some(false));
            continue;
        }

        if let Some(default_id) = default_id {
            if *id == default_id {
                gpu.set_default(Some(true));
            } else {
                gpu.set_default(Some(false));
                // Virtual GPUs (e.g. virtio-gpu in qemu) are reported as VirtualGpu by Vulkan and
                // don't count as discrete. Keep the historical behavior of treating a non-default
                // virtual GPU as a dGPU.
                if gpu.is_virtual() && !gpu.is_discrete() {
                    gpu.set_discrete(true);
                }
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
