//! Listen for PCI and DRM changes and keep Cardwire's hardware view in sync.

use std::time::Duration;

use log::{debug, error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, time::Instant
};

use crate::interface::{DebugInterface, ModeInterface};
use zbus::object_server::InterfaceRef;

const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(250);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const DISPLAY_RESTORE_QUIET_PERIOD: Duration = Duration::from_secs(5);

#[derive(Default)]
struct DisplayRestoreGuard {
    deadline: Option<Instant>,
}

impl DisplayRestoreGuard {
    fn defer(&mut self, now: Instant, restart: bool) {
        if restart || self.deadline.is_none() {
            self.deadline = Some(now + DISPLAY_RESTORE_QUIET_PERIOD);
        }
    }

    fn cancel(&mut self) -> bool {
        self.deadline.take().is_some()
    }

    fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn is_pending(&self) -> bool {
        self.deadline.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopologyChange {
    Initialize,
    Connected,
    Disconnected,
    Unchanged,
}

fn topology_change(previous: Option<bool>, connected: bool) -> TopologyChange {
    match previous {
        None => TopologyChange::Initialize,
        Some(false) if connected => TopologyChange::Connected,
        Some(true) if !connected => TopologyChange::Disconnected,
        Some(_) => TopologyChange::Unchanged,
    }
}

struct DisplayMonitorState {
    last_connected: Option<bool>,
    restore: DisplayRestoreGuard,
}

impl DisplayMonitorState {
    fn new(last_connected: Option<bool>) -> Self {
        Self {
            last_connected,
            restore: DisplayRestoreGuard::default(),
        }
    }
}

async fn apply_automatic_mode(
    mode: &ModeInterface,
    interface: &InterfaceRef<ModeInterface>,
    connected: bool,
) -> bool {
    match mode.apply_external_display_mode(connected).await {
        Ok(changed) => {
            if let Err(err) = mode.emit_mode_change(interface, changed).await {
                error!("failed to emit automatic mode change: {err}");
            }
            true
        }
        Err(err) => {
            warn!("failed to apply automatic external-display mode: {err}");
            false
        }
    }
}

async fn reconcile_display_topology(
    mode: &ModeInterface,
    interface: &InterfaceRef<ModeInterface>,
    state: &mut DisplayMonitorState,
    topology_event: bool,
) {
    let connected = match mode.required_external_display_connected().await {
        Ok(connected) => connected,
        Err(err) => {
            warn!("failed to inspect external display topology: {err}");
            return;
        }
    };

    if !mode.external_display_auto_switch_enabled() {
        if state.restore.cancel() {
            info!("external-display mode restoration canceled because auto-switch is disabled");
        }
        state.last_connected = Some(connected);
        return;
    }

    match topology_change(state.last_connected, connected) {
        TopologyChange::Initialize | TopologyChange::Connected => {
            if state.restore.cancel() {
                info!("external display reconnected; pending mode restoration canceled");
            }
            if apply_automatic_mode(mode, interface, connected).await {
                state.last_connected = Some(connected);
            }
        }
        TopologyChange::Disconnected => {
            state.last_connected = Some(false);
            state.restore.defer(Instant::now(), true);
            info!(
                "external dGPU display disconnected; restoring the configured mode after {} seconds",
                DISPLAY_RESTORE_QUIET_PERIOD.as_secs()
            );
        }
        TopologyChange::Unchanged => {
            if connected && topology_event {
                if !apply_automatic_mode(mode, interface, true).await {
                    state.last_connected = Some(false);
                }
            } else if !connected && state.restore.is_pending() && topology_event {
                state.restore.defer(Instant::now(), true);
                debug!(
                    "DRM topology changed while mode restoration was pending; restarting the quiet period"
                );
            }
        }
    }
}

async fn restore_mode_after_settle(
    mode: &ModeInterface,
    interface: &InterfaceRef<ModeInterface>,
    state: &mut DisplayMonitorState,
) {
    state.restore.cancel();

    if !mode.external_display_auto_switch_enabled() {
        return;
    }

    let connected = match mode.required_external_display_connected().await {
        Ok(connected) => connected,
        Err(err) => {
            warn!("failed to verify external display topology: {err}");
            state.restore.defer(Instant::now(), false);
            return;
        }
    };

    if connected {
        info!("external display reconnected before mode restoration");
        if apply_automatic_mode(mode, interface, true).await {
            state.last_connected = Some(true);
        } else {
            state.last_connected = Some(false);
        }
        return;
    }

    info!("external display topology settled; applying the configured restore mode");
    if apply_automatic_mode(mode, interface, false).await {
        state.last_connected = Some(false);
    } else {
        state.restore.defer(Instant::now(), false);
        warn!(
            "configured mode restoration failed; retrying in {} seconds",
            DISPLAY_RESTORE_QUIET_PERIOD.as_secs()
        );
    }
}

pub async fn monitor_hardware_changes(
    debug_int: DebugInterface,
    mode: ModeInterface,
    mode_interface: InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    let pci_monitor = udev::MonitorBuilder::new()?.match_subsystem("pci")?;
    let pci_fd = AsyncFd::new(pci_monitor.listen()?)?;
    let drm_monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?;
    let drm_fd = AsyncFd::new(drm_monitor.listen()?)?;
    let mut retry = tokio::time::interval(RETRY_INTERVAL);
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let initial_connected = mode.required_external_display_connected().await.ok();
    let mut display_state = DisplayMonitorState::new(initial_connected);

    loop {
        let restore_deadline = display_state
            .restore
            .deadline()
            .unwrap_or_else(Instant::now);
        tokio::select! {
            ready = pci_fd.ready(Interest::READABLE) => {
                let mut guard = ready?;
                let mut refresh = false;
                if guard.ready().is_readable() {
                    for event in pci_fd.get_ref().iter() {
                        if event.action().is_some_and(|action| action == "bind" || action == "unbind") {
                            refresh = true;
                        }
                    }
                }
                guard.clear_ready();
                drop(guard);

                if refresh {
                    info!("detected PCI event, refreshing GPU interfaces");
                    if let Err(err) = debug_int.refresh_gpu().await {
                        error!("failed to refresh GPU interface: {err}");
                    }
                    tokio::time::sleep(DISPLAY_DEBOUNCE).await;
                    reconcile_display_topology(
                        &mode,
                        &mode_interface,
                        &mut display_state,
                        true,
                    )
                    .await;
                }
            }
            ready = drm_fd.ready(Interest::READABLE) => {
                let mut guard = ready?;
                let mut reconcile = false;
                if guard.ready().is_readable() {
                    for event in drm_fd.get_ref().iter() {
                        if event.action().is_some_and(|action| {
                            action == "add" || action == "remove" || action == "change"
                        }) {
                            reconcile = true;
                        }
                    }
                }
                guard.clear_ready();
                drop(guard);

                if reconcile {
                    tokio::time::sleep(DISPLAY_DEBOUNCE).await;
                    reconcile_display_topology(
                        &mode,
                        &mode_interface,
                        &mut display_state,
                        true,
                    )
                    .await;
                }
            }
            _ = retry.tick() => {
                reconcile_display_topology(
                    &mode,
                    &mode_interface,
                    &mut display_state,
                    false,
                )
                .await;
            }
            _ = tokio::time::sleep_until(restore_deadline), if display_state.restore.is_pending() => {
                restore_mode_after_settle(&mode, &mode_interface, &mut display_state).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_changes_are_edge_triggered() {
        assert_eq!(topology_change(None, false), TopologyChange::Initialize);
        assert_eq!(
            topology_change(Some(false), true),
            TopologyChange::Connected
        );
        assert_eq!(
            topology_change(Some(true), false),
            TopologyChange::Disconnected
        );
        assert_eq!(topology_change(Some(true), true), TopologyChange::Unchanged);
        assert_eq!(
            topology_change(Some(false), false),
            TopologyChange::Unchanged
        );
    }

    #[test]
    fn stable_topology_does_not_create_an_automatic_edge() {
        assert_eq!(topology_change(Some(true), true), TopologyChange::Unchanged);
        assert_eq!(
            topology_change(Some(false), false),
            TopologyChange::Unchanged
        );
    }

    #[test]
    fn topology_event_restarts_restore_quiet_period() {
        let start = Instant::now();
        let mut guard = DisplayRestoreGuard::default();
        guard.defer(start, true);
        let first_deadline = guard.deadline().unwrap();

        let second_event = start + Duration::from_secs(1);
        guard.defer(second_event, true);

        assert_eq!(
            guard.deadline(),
            Some(second_event + DISPLAY_RESTORE_QUIET_PERIOD)
        );
        assert!(guard.deadline().unwrap() > first_deadline);
    }

    #[test]
    fn retry_does_not_extend_pending_restore() {
        let start = Instant::now();
        let mut guard = DisplayRestoreGuard::default();
        guard.defer(start, false);
        guard.defer(start, false);
        let deadline = guard.deadline();

        guard.defer(start + Duration::from_secs(2), false);

        assert_eq!(guard.deadline(), deadline);
    }

    #[test]
    fn reconnect_cancels_pending_restore() {
        let mut guard = DisplayRestoreGuard::default();
        guard.defer(Instant::now(), false);

        assert!(guard.cancel());
        assert!(!guard.is_pending());
        assert!(!guard.cancel());
    }
}
