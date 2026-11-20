use std::{path::Path, thread, time::Duration};

use log::{info, warn};

/// Wait for the device to be initialized
pub fn wait_for_drm(pci_id: &str, retries: usize) -> bool {
    let drm = Path::new("/sys/bus/pci/devices").join(pci_id).join("drm");

    for attempt in 0..retries {
        warn!(
            "[{}/{}] waiting for {} DRM subsystem to init...",
            attempt, retries, pci_id
        );
        if drm.read_dir().is_ok_and(|mut d| d.next().is_some()) {
            info!("{} DRM subsystem is ready", pci_id);
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

/// Get the drm nodes using sysfs
pub fn sysfs_get_device_drm(pci_id: &str) -> Option<(u32, u32)> {
    let syspath = Path::new("/sys/bus/pci/devices").join(pci_id).join("drm");
    let drm = syspath.read_dir().ok()?;

    let mut render: Option<u32> = None;
    let mut card: Option<u32> = None;

    for entry in drm.flatten() {
        if let Some(str) = entry.file_name().to_str()
            && str.contains("card")
        {
            let minor_s_opt = str.strip_prefix("card");
            if let Some(minor_s) = minor_s_opt
                && let Ok(minor_int) = minor_s.parse::<u32>()
            {
                card = Some(minor_int);
                continue;
            }
        }
        if let Some(str) = entry.file_name().to_str()
            && str.contains("renderD")
        {
            let minor_s_opt = str.strip_prefix("renderD");
            if let Some(minor_s) = minor_s_opt
                && let Ok(minor_int) = minor_s.parse::<u32>()
            {
                render = Some(minor_int);
                continue;
            }
        }
    }

    Some((card?, render?))
}
