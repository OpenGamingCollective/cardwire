//! DRM display connector detection and node resolution.

use log::{info, warn};
use std::{env, fs, io, path::Path, time::Duration};
use udev::{Device, Enumerator};

use crate::core::desktop::Desktop;

const NON_PHYSICAL: &[&str] = &["Virtual-", "Unknown-", "Writeback-"];
const INTERNAL_PANELS: &[&str] = &["eDP-", "LVDS-", "DSI-", "DPI-", "SPI-"];

/// Return whether a DRM card currently owns a connected physical external display.
///
/// Connector ownership is encoded in sysfs names such as `card1-HDMI-A-1`. Internal panels and
/// virtual connectors are excluded so only physical external outputs keep the card available.
#[allow(dead_code)]
pub fn external_display_connected(card: u32) -> io::Result<bool> {
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
            || NON_PHYSICAL
                .iter()
                .any(|prefix| connector.starts_with(prefix))
            || INTERNAL_PANELS
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

/// Check whether the given DRM card currently has any connected display.
///
/// Reads `/sys/class/drm/card{card}-*/status`
pub async fn is_gpu_active(card: u32) -> Option<bool> {
    let prefix = format!("card{card}-");
    let mut entries = tokio::fs::read_dir("/sys/class/drm").await.ok()?;
    let mut status_error = None;
    while let Some(entry) = entries.next_entry().await.ok()? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(connector) = name.strip_prefix(&prefix) else {
            continue;
        };
        if connector.is_empty()
            || NON_PHYSICAL
                .iter()
                .any(|prefix| connector.starts_with(prefix))
        {
            continue;
        }
        match tokio::fs::read_to_string(entry.path().join("status")).await {
            Ok(status) if status.trim() == "connected" => return Some(true),
            Ok(_) => {}
            Err(err) => {
                status_error.get_or_insert(err);
            }
        }
    }
    match status_error {
        Some(_) => None,
        None => Some(false),
    }
}

/// Uevent action sent to a DRM card.
pub enum UdevAction {
    Add,
    Remove,
}

/// Send a uevent for a DRM card, prompting the display server to react to it.
pub async fn send_drm_uevent(card: u32, action: UdevAction) -> io::Result<()> {
    match action {
        UdevAction::Add => {
            tokio::fs::write(format!("/sys/class/drm/card{card}/uevent"), "add\n").await
        }
        UdevAction::Remove => {
            let desktop_str: String = match env::var("XDG_CURRENT_DESKTOP") {
                Ok(value) => value,
                Err(_) => return Ok(()),
            };
            // Mutter has no device removal: a "remove" event only makes it rescan the blocked
            // card, hitting its stale-pointer crash path. No-op until fixed upstream.
            if Desktop::from_str(&desktop_str).is_some_and(|d| d == Desktop::Gnome) {
                Ok(())
            } else {
                tokio::fs::write(format!("/sys/class/drm/card{card}/uevent"), "remove\n").await
            }
        }
    }
}
