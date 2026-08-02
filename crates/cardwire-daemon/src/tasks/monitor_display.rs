//! Apply mode overrides for displays connected to dGPU-only ports.
//!
//! The persisted mode remains the user's requested mode. When an external display needs the dGPU,
//! the effective mode is temporarily changed to Hybrid and restored after the display disconnects.

use std::{
    collections::BTreeMap, sync::{Arc, atomic::Ordering}, time::Duration
};

use log::{error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock}
};
use zbus::{fdo, object_server::InterfaceRef};

use crate::{
    core::gpu::external_display_connected, file::CardwireModeState, interface::{ConfigMemory, GpuInterface, ModeInterface, Modes}
};

// Give connector status files time to settle after a burst of DRM uevents.
const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(250);
// Reconcile periodically as a fallback for missed uevents and runtime configuration changes.
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
// Avoid blocking the dGPU immediately while a dock or display is still tearing down.
const DISPLAY_RESTORE_WAIT: Duration = Duration::from_secs(5);

/// Coordinates requested modes with the mode currently applied to the GPUs.
#[derive(Clone)]
pub struct DisplayMode {
    // Persisted user choice, which must survive a temporary external-display override.
    mode_state: Arc<RwLock<CardwireModeState>>,
    gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    config: Arc<ConfigMemory>,
    // Serialize startup, D-Bus, and monitor transitions so hardware state changes cannot overlap.
    transition: Arc<Mutex<()>>,
    // Mode applied to the GPUs and exposed over D-Bus; it may differ from `mode_state`.
    effective_mode: Arc<RwLock<Modes>>,
}

impl DisplayMode {
    /// Create a display-mode coordinator seeded with the persisted requested mode.
    pub async fn new(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
    ) -> Self {
        // Startup mode application will replace this value after the GPU interfaces are ready.
        let effective_mode = mode_state.read().await.mode();
        Self {
            mode_state,
            gpu_list,
            config,
            transition: Arc::new(Mutex::new(())),
            effective_mode: Arc::new(RwLock::new(effective_mode)),
        }
    }

    /// Return the mode currently applied to the GPU state and eBPF map.
    pub async fn current_mode(&self) -> Modes {
        *self.effective_mode.read().await
    }

    /// Choose the effective mode for the requested mode and current display topology.
    fn external_display_target(requested: Modes, connected: bool) -> Modes {
        // Integrated and Smart may block the non-default GPU; Hybrid keeps both GPUs available.
        if connected && matches!(requested, Modes::Integrated | Modes::Smart) {
            Modes::Hybrid
        } else {
            requested
        }
    }

    /// Return whether external-display switching applies to the requested mode.
    fn auto_switch_active(&self, requested: Modes) -> bool {
        self.config
            .external_display_auto_switch
            .load(Ordering::Relaxed)
            && matches!(requested, Modes::Integrated | Modes::Smart)
    }

    /// Resolve the effective mode and the connected card which requires an override.
    async fn target(&self, requested: Modes) -> fdo::Result<(Modes, Option<u32>)> {
        if !self.auto_switch_active(requested) {
            return Ok((requested, None));
        }

        let card = {
            let gpu_list = self.gpu_list.read().await;
            // These automatic modes already require the daemon's supported two-GPU layout.
            if gpu_list.len() != 2 {
                return Ok((requested, None));
            }
            // Mode application treats the non-default GPU as the discrete GPU.
            gpu_list
                .values()
                .find(|gpu| !gpu.device.is_default())
                .map(|gpu| *gpu.device.card())
        };
        let connected = match card {
            Some(card) => external_display_connected(card).map_err(|err| {
                fdo::Error::Failed(format!("failed to read DRM connector state: {err}"))
            })?,
            None => false,
        };

        // Return the card only while its display is connected; callers use it both to track the
        // topology edge and to replay the DRM event after unblocking the card.
        Ok((
            Self::external_display_target(requested, connected),
            card.filter(|_| connected),
        ))
    }

    /// Apply an effective mode when needed and report whether its observable value changed.
    async fn apply_target(
        &self,
        mode: &ModeInterface,
        requested: Modes,
        target: Modes,
        card: Option<u32>,
        force: bool,
    ) -> fdo::Result<bool> {
        let previous = self.current_mode().await;
        if force || target != previous {
            // Publish the effective mode only after every GPU and the eBPF map were updated.
            mode.apply_mode(target).await?;
            *self.effective_mode.write().await = target;
        }
        // The original hotplug event may have arrived while access to this DRM card was blocked.
        // Replay it after switching to Hybrid so compositors can discover the connector again.
        if target != previous
            && target == Modes::Hybrid
            && requested != Modes::Hybrid
            && let Some(card) = card
        {
            let path = format!("/sys/class/drm/card{card}/uevent");
            if let Err(err) = tokio::fs::write(&path, "change\n").await {
                warn!("failed to replay DRM change event through {path}: {err}");
            }
        }
        Ok(target != previous)
    }

    /// Apply a daemon-owned mode transition without persisting the requested mode.
    pub async fn apply(
        &self,
        mode: &ModeInterface,
        requested: Modes,
        force: bool,
    ) -> fdo::Result<bool> {
        let _transition = self.transition.lock().await;
        let (target, card) = self.target(requested).await?;
        self.apply_target(mode, requested, target, card, force)
            .await
    }

    /// Apply and persist a mode explicitly requested through D-Bus.
    pub async fn set(&self, mode: &ModeInterface, requested: Modes) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        let (target, card) = self.target(requested).await?;
        self.apply_target(mode, requested, target, card, false)
            .await?;
        mode.save_mode(requested).await;
        Ok(())
    }

    /// Apply the persisted mode during daemon startup, including any required display override.
    pub async fn apply_at_startup(
        &self,
        mode: &ModeInterface,
        requested: Modes,
    ) -> fdo::Result<()> {
        let _transition = self.transition.lock().await;
        // Topology discovery must not prevent startup. The monitor will retry once D-Bus is live.
        let (target, card) = match self.target(requested).await {
            Ok(target) => target,
            Err(err) => {
                warn!("failed to read external display topology at startup: {err}");
                (requested, None)
            }
        };
        self.apply_target(mode, requested, target, card, true)
            .await?;
        Ok(())
    }

    /// Reconcile the persisted request with current topology and delay restoration after unplug.
    async fn reconcile(
        &self,
        mode: &ModeInterface,
        wait_for_disconnect: bool,
    ) -> fdo::Result<(bool, Option<u32>)> {
        let mut transition = self.transition.lock().await;
        // Always read the persisted request again; a D-Bus setter may have changed it since the
        // previous monitor iteration.
        let mut requested = self.mode_state.read().await.mode();
        let (mut target, mut card) = self.target(requested).await?;

        if wait_for_disconnect && card.is_none() && self.auto_switch_active(requested) {
            // Do not make explicit D-Bus mode changes wait behind the disconnect grace period.
            drop(transition);
            info!(
                "external dGPU display disconnected; restoring the configured mode after {} seconds",
                DISPLAY_RESTORE_WAIT.as_secs()
            );
            tokio::time::sleep(DISPLAY_RESTORE_WAIT).await;
            // Both the requested mode and topology may have changed while the lock was released.
            transition = self.transition.lock().await;
            requested = self.mode_state.read().await.mode();
            (target, card) = self.target(requested).await?;
            if card.is_some() {
                info!("external display reconnected; keeping the current mode");
            }
        }

        let changed = self
            .apply_target(mode, requested, target, card, false)
            .await?;
        drop(transition);
        Ok((changed, card))
    }
}

/// Monitor DRM uevents and periodic retries, applying and signaling automatic mode changes.
async fn run_display_monitor(
    mode: &ModeInterface,
    display_mode: &DisplayMode,
    interface: &InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    let drm_monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?;
    let drm_fd = AsyncFd::new(drm_monitor.listen()?)?;
    let mut retry = tokio::time::interval(RETRY_INTERVAL);
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // This records whether the previous successful reconciliation required an override. It enables
    // the disconnect grace period without persisting temporary topology state.
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
        match display_mode.reconcile(mode, connected).await {
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
    display_mode: DisplayMode,
    mode_interface: InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    loop {
        if let Err(err) = run_display_monitor(&mode, &display_mode, &mode_interface).await {
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
            assert_eq!(
                DisplayMode::external_display_target(mode, true),
                Modes::Hybrid
            );
        }
        for mode in [Modes::Hybrid, Modes::Manual] {
            assert_eq!(DisplayMode::external_display_target(mode, true), mode);
        }
        for mode in [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ] {
            assert_eq!(DisplayMode::external_display_target(mode, false), mode);
        }
    }
}
