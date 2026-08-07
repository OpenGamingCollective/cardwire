//! Apply mode overrides for displays connected to dGPU-only ports.
//!
//! The persisted mode remains the user's requested mode. When an external display needs the dGPU,
//! the effective mode is temporarily changed to Hybrid and restored after the display disconnects.

use std::{
    collections::{BTreeMap, HashSet}, sync::Arc, time::Duration
};

use log::warn;
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::RwLock
};

use crate::{
    core::gpu::is_gpu_active, file::CardwireModeState, interface::{GpuInterface, ModeInterface, Modes}
};

// Give connector status files time to settle after a burst of DRM uevents.
const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(250);
// Reconcile periodically as a fallback for missed uevents and runtime configuration changes.
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Extract the DRM card number from a uevent device (e.g. `card1` -> 1).
fn card_from_event(event: &udev::Event) -> Option<u32> {
    event
        .device()
        .sysname()
        .to_string_lossy()
        .strip_prefix("card")?
        .parse::<u32>()
        .ok()
}

/// Find the GPU interface matching a DRM card number, returning its map key and interface.
fn find_gpu_by_card(
    card: u32,
    gpu_list: &BTreeMap<usize, Arc<GpuInterface>>,
) -> Option<(usize, &Arc<GpuInterface>)> {
    gpu_list
        .iter()
        .find(|(_, gpu)| *gpu.device.card() == card)
        .map(|(id, gpu)| (*id, gpu))
}

/// Reconcile one DRM card against the current mode and its GPU's block state.
///
/// The persisted mode is never touched: when the offload dGPU needs to drive a connected
/// display, the effective mode is temporarily overridden to Hybrid and restored once the
/// display disconnects.
async fn reconcile_gpu(
    card: u32,
    mode: &ModeInterface,
    gpu_list: &Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
) {
    let Some(active) = is_gpu_active(card).await else {
        warn!("failed to probe display state for card{card}; skipping reconcile");
        return;
    };
    let Some(current_mode) = mode
        .mode()
        .await
        .ok()
        .and_then(|mode| Modes::try_from(mode).ok())
    else {
        return;
    };

    match current_mode {
        // Integrated and Smart modes keep the offload dGPU blocked, except when it needs to
        // drive a connected display: then the effective mode is overridden to Hybrid.
        Modes::Integrated | Modes::Smart => {
            let gpu_list_read = gpu_list.read().await;
            // Only the offload dGPU (discrete, not the default display) is affected.
            let Some((_, gpu)) = find_gpu_by_card(card, &gpu_list_read) else {
                return;
            };
            if !gpu.device.is_discrete() || gpu.device.is_default() {
                return;
            }
            let blocked = gpu.gpu_blocked().await.unwrap_or(false);
            drop(gpu_list_read);

            if active && blocked {
                // The dGPU must drive the display: unblock it by switching to Hybrid.
                let _ = mode.internal_set_mode(Modes::Hybrid, Some(false)).await;
            } else if !active && !blocked {
                // The override is in effect and the display is gone: restore the persisted mode
                // from disk, since mode() now reports the applied override.
                if let Ok(persisted) = CardwireModeState::build() {
                    let _ = mode.internal_set_mode(persisted.mode(), Some(false)).await;
                }
            }
        }
        // Manual mode leaves the user in control, except unblocking a GPU that suddenly
        // needs to drive a connected display.
        Modes::Manual => {
            let gpu_list_read = gpu_list.read().await;
            let Some((_, gpu)) = find_gpu_by_card(card, &gpu_list_read) else {
                return;
            };
            if active && gpu.gpu_blocked().await.unwrap_or(false) {
                // Unblocking does not touch the GPU list, clone and drop the guard first.
                let gpu = Arc::clone(gpu);
                drop(gpu_list_read);
                let _ = gpu.unblock_gpu().await;
            }
        }
        // Hybrid mode keeps every GPU unblocked, nothing to reconcile.
        Modes::Hybrid => {}
    }
}

/// Monitor DRM uevents and periodic retries, applying and signaling automatic mode changes.
async fn run_display_monitor(
    mode: &ModeInterface,
    gpu_list: &Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
) -> zbus::Result<()> {
    let drm_monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?;
    let drm_fd = AsyncFd::new(drm_monitor.listen()?)?;
    let mut retry = tokio::time::interval(RETRY_INTERVAL);
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut cards_seen: HashSet<u32> = HashSet::new();

    loop {
        let topology_event = tokio::select! {
            ready = drm_fd.ready(Interest::READABLE) => {
                let mut guard = ready?;
                if guard.ready().is_readable() {
                    // Drain the socket and collapse a burst of uevents into one reconciliation.
                    for event in drm_fd.get_ref().iter() {
                        if event.action().is_some_and(|action| {
                            action == "add" || action == "remove" || action == "change"
                        }) && let Some(card) = card_from_event(&event) {
                            cards_seen.insert(card);
                        }
                    }
                }
                guard.clear_ready();
                !cards_seen.is_empty()
            }
            _ = retry.tick() => {
                // On retry, check all GPUs as a fallback for missed uevents.
                let gpu_list_read = gpu_list.read().await;
                for gpu in gpu_list_read.values() {
                    cards_seen.insert(*gpu.device.card());
                }
                !cards_seen.is_empty()
            }
        };

        if topology_event {
            // Connector status can lag behind the event which announced the topology change.
            tokio::time::sleep(DISPLAY_DEBOUNCE).await;
            let cards = std::mem::take(&mut cards_seen);
            for card in cards {
                reconcile_gpu(card, mode, gpu_list).await;
            }
        }
    }
}

/// Keep the display monitor alive by recreating it after recoverable failures.
pub async fn monitor_display_changes(
    mode: ModeInterface,
    gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
) -> zbus::Result<()> {
    loop {
        if let Err(err) = run_display_monitor(&mode, &gpu_list).await {
            warn!(
                "display monitor exited with error: {err}; retrying in {} seconds",
                RETRY_INTERVAL.as_secs()
            );
            tokio::time::sleep(RETRY_INTERVAL).await;
        }
    }
}
