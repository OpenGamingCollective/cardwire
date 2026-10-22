//! DRM display connector detection and node resolution.

use std::{fs, io};

const NON_PHYSICAL: &[&str] = &["Virtual-", "Unknown-", "Writeback-"];
const INTERNAL_PANELS: &[&str] = &["eDP-", "LVDS-", "DSI-", "DPI-", "SPI-"];

/// Return whether a DRM card currently owns a connected physical external display.
#[allow(dead_code)]
pub fn external_display_connected(card: u32) -> io::Result<bool> {
    let card_prefix = format!("card{card}-");
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
            Ok(status) if status.trim() == "connected" => return Ok(true),
            Ok(_) => {}
            Err(err) => {
                status_error.get_or_insert(err);
            }
        }
    }

    match status_error {
        Some(err) => Err(err),
        None => Ok(false),
    }
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
