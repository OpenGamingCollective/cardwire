//! Apply mode overrides for displays connected to dGPU-only ports.
//!
//! The persisted mode remains the user's requested mode. When an external display needs the dGPU,
//! the effective mode is temporarily changed to Hybrid and restored after the display disconnects.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use log::{error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::RwLock
};
use zbus::{fdo, object_server::InterfaceRef};

use crate::interface::{ConfigMemory, GpuInterface, ModeInterface, Modes};

// Give connector status files time to settle after a burst of DRM uevents.
const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(250);
// Reconcile periodically as a fallback for missed uevents and runtime configuration changes.
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
// Avoid blocking the dGPU immediately while a dock or display is still tearing down.
const DISPLAY_RESTORE_WAIT: Duration = Duration::from_secs(5);

/// Choose the effective mode for the requested mode and current display topology.
pub fn external_display_target(requested: Modes, connected: bool) -> Modes {
    // Integrated and Smart may block the non-default GPU; Hybrid keeps both GPUs available.
    if connected && matches!(requested, Modes::Integrated | Modes::Smart) {
        Modes::Hybrid
    } else {
        requested
    }
}

/// Determine target effective mode and DRM card taking into account external display state.
pub(crate) async fn detect_external_display_target(
    gpu_list: &Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    config: &Arc<ConfigMemory>,
    requested: Modes,
) -> fdo::Result<(Modes, Option<u32>)> {
    let auto_switch = config
        .external_display_auto_switch
        .load(std::sync::atomic::Ordering::Relaxed);
    if !auto_switch || !matches!(requested, Modes::Integrated | Modes::Smart) {
        return Ok((requested, None));
    }

    let card = {
        let gpu_list = gpu_list.read().await;
        if gpu_list.len() != 2 {
            return Ok((requested, None));
        }
        gpu_list
            .values()
            .find(|gpu| !gpu.device.is_default())
            .map(|gpu| *gpu.device.card())
    };

    let connected = match card {
        Some(card) => {
            tokio::task::spawn_blocking(move || crate::core::gpu::external_display_connected(card))
                .await
                .map_err(|err| fdo::Error::Failed(format!("DRM probe task failed: {err}")))?
                .map_err(|err| {
                    fdo::Error::Failed(format!("failed to read DRM connector state: {err}"))
                })?
        }
        None => false,
    };

    let target = external_display_target(requested, connected);

    Ok((target, card.filter(|_| connected)))
}

/// Reconcile the persisted request with current display topology.
async fn reconcile_display_mode(
    mode: &ModeInterface,
    was_connected: bool,
) -> fdo::Result<(bool, Option<u32>)> {
    let mut requested = mode.requested_mode_value().await;
    let (mut target, mut card) = mode.detect_display_target(requested).await?;

    if was_connected && card.is_none() && matches!(requested, Modes::Integrated | Modes::Smart) {
        info!(
            "external dGPU display disconnected; restoring the configured mode after {} seconds",
            DISPLAY_RESTORE_WAIT.as_secs()
        );
        tokio::time::sleep(DISPLAY_RESTORE_WAIT).await;
        requested = mode.requested_mode_value().await;
        (target, card) = mode.detect_display_target(requested).await?;
        if card.is_some() {
            info!("external display reconnected; keeping the current mode");
        }
    }

    let changed = mode.effective_set_mode(target, false).await?;

    if changed
        && target == Modes::Hybrid
        && requested != Modes::Hybrid
        && let Some(card) = card
    {
        let path = format!("/sys/class/drm/card{card}/uevent");
        if let Err(err) = tokio::fs::write(&path, "change\n").await {
            warn!("failed to replay DRM change event through {path}: {err}");
        }
    }

    Ok((changed, card))
}

/// Monitor DRM uevents and periodic retries, applying and signaling automatic mode changes.
async fn run_display_monitor(
    mode: &ModeInterface,
    interface: &InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    let drm_monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?;
    let drm_fd = AsyncFd::new(drm_monitor.listen()?)?;
    let mut retry = tokio::time::interval(RETRY_INTERVAL);
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Records whether the previous successful reconciliation had a connected external display.
    let mut connected = false;

    loop {
        let topology_event = tokio::select! {
            ready = drm_fd.ready(Interest::READABLE) => {
                let mut guard = ready?;
                let mut changed = false;
                if guard.ready().is_readable() {
                    // Drain the socket and collapse a burst of uevents into one reconciliation.
                    for event in drm_fd.get_ref().iter() {
                        changed |= event.action().is_some_and(|action| {
                            action == "add" || action == "remove" || action == "change"
                        });
                    }
                }
                guard.clear_ready();
                changed
            }
            _ = retry.tick() => false,
        };

        if topology_event {
            // Connector status can lag behind the event which announced the topology change.
            tokio::time::sleep(DISPLAY_DEBOUNCE).await;
        }
        match reconcile_display_mode(mode, connected).await {
            Ok((changed, card)) => {
                connected = card.is_some();
                // Automatic transitions bypass the D-Bus property setter, so emit its signal here.
                if let Err(err) = mode.emit_mode_change(interface, changed).await {
                    error!("failed to emit automatic mode change: {err}");
                }
            }
            Err(err) => warn!("failed to reconcile external display mode: {err}"),
        }
    }
}

/// Keep the display monitor alive by recreating it after recoverable failures.
pub async fn monitor_display_changes(
    mode: ModeInterface,
    mode_interface: InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    loop {
        if let Err(err) = run_display_monitor(&mode, &mode_interface).await {
            warn!(
                "display monitor exited with error: {err}; retrying in {} seconds",
                RETRY_INTERVAL.as_secs()
            );
            tokio::time::sleep(RETRY_INTERVAL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_display_only_overrides_integrated_and_smart() {
        for mode in [Modes::Integrated, Modes::Smart] {
            assert_eq!(external_display_target(mode, true), Modes::Hybrid);
        }
        for mode in [Modes::Hybrid, Modes::Manual] {
            assert_eq!(external_display_target(mode, true), mode);
        }
        for mode in [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ] {
            assert_eq!(external_display_target(mode, false), mode);
        }
    }
}
