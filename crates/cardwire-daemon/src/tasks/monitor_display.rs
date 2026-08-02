//! Monitor DRM topology and apply automatic external-display mode changes.

use std::time::Duration;

use log::{error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, time::Instant
};
use zbus::object_server::InterfaceRef;

use crate::interface::ModeInterface;

const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(250);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const DISPLAY_RESTORE_QUIET_PERIOD: Duration = Duration::from_secs(5);

#[derive(Default)]
struct DisplayMonitorState {
    last_connected: Option<bool>,
    restore_deadline: Option<Instant>,
}

impl DisplayMonitorState {
    fn record_topology(
        &mut self,
        connected: bool,
        topology_event: bool,
        enabled: bool,
        now: Instant,
    ) -> bool {
        let previous = self.last_connected.replace(connected);

        if !enabled {
            self.restore_deadline = None;
            return true;
        }

        if connected {
            self.restore_deadline = None;
            return true;
        }

        if previous.is_none()
            || previous == Some(true)
            || (topology_event && self.restore_deadline.is_some())
        {
            self.restore_deadline = Some(now + DISPLAY_RESTORE_QUIET_PERIOD);
            false
        } else {
            self.restore_deadline.is_none()
        }
    }
}

async fn apply_automatic_mode(mode: &ModeInterface, interface: &InterfaceRef<ModeInterface>) {
    match mode.reconcile_external_display().await {
        Ok(changed) => {
            if let Err(err) = mode.emit_mode_change(interface, changed).await {
                error!("failed to emit automatic mode change: {err}");
            }
        }
        Err(err) => warn!("failed to apply automatic external-display mode: {err}"),
    }
}

async fn observe_topology(
    mode: &ModeInterface,
    interface: &InterfaceRef<ModeInterface>,
    state: &mut DisplayMonitorState,
    topology_event: bool,
) {
    let enabled = mode.external_display_monitor_enabled().await;
    let connected = if enabled {
        match mode.required_external_display_connected().await {
            Ok(connected) => connected,
            Err(err) => {
                warn!("failed to inspect external display topology: {err}");
                return;
            }
        }
    } else {
        false
    };
    let previous = state.last_connected;
    let reconcile = state.record_topology(connected, topology_event, enabled, Instant::now());

    if enabled && previous == Some(true) && !connected {
        info!(
            "external dGPU display disconnected; restoring the configured mode after {} seconds",
            DISPLAY_RESTORE_QUIET_PERIOD.as_secs()
        );
    }
    if previous == Some(false) && connected {
        info!("external display reconnected; pending mode restoration canceled");
    }
    if reconcile {
        apply_automatic_mode(mode, interface).await;
    }
}

async fn run_display_monitor(
    mode: &ModeInterface,
    mode_interface: &InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    let drm_monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?;
    let drm_fd = AsyncFd::new(drm_monitor.listen()?)?;
    let mut retry = tokio::time::interval(RETRY_INTERVAL);
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut state = DisplayMonitorState::default();

    observe_topology(mode, mode_interface, &mut state, false).await;

    loop {
        let restore_deadline = state.restore_deadline.unwrap_or_else(Instant::now);
        tokio::select! {
            ready = drm_fd.ready(Interest::READABLE) => {
                let mut guard = ready?;
                let mut topology_event = false;
                if guard.ready().is_readable() {
                    for event in drm_fd.get_ref().iter() {
                        topology_event |= event.action().is_some_and(|action| {
                            action == "add" || action == "remove" || action == "change"
                        });
                    }
                }
                guard.clear_ready();
                drop(guard);

                if topology_event {
                    tokio::time::sleep(DISPLAY_DEBOUNCE).await;
                    observe_topology(mode, mode_interface, &mut state, true).await;
                }
            }
            _ = retry.tick() => {
                observe_topology(mode, mode_interface, &mut state, false).await;
            }
            _ = tokio::time::sleep_until(restore_deadline), if state.restore_deadline.is_some() => {
                state.restore_deadline = None;
                observe_topology(mode, mode_interface, &mut state, false).await;
            }
        }
    }
}

pub async fn monitor_display_changes(
    mode: ModeInterface,
    mode_interface: InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    loop {
        if let Err(err) = run_display_monitor(&mode, &mode_interface).await {
            warn!(
                "display monitor task exited with error: {err}; retrying in {} seconds",
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
    fn disconnect_waits_and_repeated_event_restarts_deadline() {
        let start = Instant::now();
        let mut state = DisplayMonitorState {
            last_connected: Some(true),
            ..DisplayMonitorState::default()
        };

        assert!(!state.record_topology(false, true, true, start));
        let first = state.restore_deadline.unwrap();
        assert!(!state.record_topology(false, true, true, start + Duration::from_secs(1)));
        assert!(state.restore_deadline.unwrap() > first);
    }

    #[test]
    fn retry_does_not_extend_pending_restore() {
        let start = Instant::now();
        let mut state = DisplayMonitorState::default();
        assert!(!state.record_topology(false, false, true, start));
        let deadline = state.restore_deadline;

        assert!(!state.record_topology(false, false, true, start + Duration::from_secs(2)));
        assert_eq!(state.restore_deadline, deadline);
    }

    #[test]
    fn reconnect_or_disable_cancels_pending_restore() {
        let start = Instant::now();
        let mut state = DisplayMonitorState::default();
        state.record_topology(false, false, true, start);

        assert!(state.record_topology(true, true, true, start));
        assert!(state.restore_deadline.is_none());

        state.record_topology(false, true, true, start);
        assert!(state.record_topology(false, false, false, start));
        assert!(state.restore_deadline.is_none());
    }
}
