//! Serialize mode requests and reconcile automatic external-display mode changes.

use std::{
    collections::BTreeMap, sync::{Arc, atomic::Ordering}, time::Duration
};

use log::{error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{RwLock, mpsc, watch}, task, time::Instant
};
use zbus::{fdo, object_server::InterfaceRef};

use crate::{
    core::gpu::external_display_connected, file::CardwireModeState, interface::{ConfigMemory, GpuInterface, ModeInterface, ModeRuntime, Modes, SetModeRequest}
};

const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(250);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const DISPLAY_RESTORE_QUIET_PERIOD: Duration = Duration::from_secs(5);

#[derive(Default)]
struct DisplayMonitorState {
    last_connected: Option<bool>,
    debounce_deadline: Option<Instant>,
    restore_deadline: Option<Instant>,
}

impl DisplayMonitorState {
    fn debounce(&mut self, now: Instant) {
        self.debounce_deadline = Some(now + DISPLAY_DEBOUNCE);
    }

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

pub struct DisplayModeTask {
    mode: ModeInterface,
    mode_state: Arc<RwLock<CardwireModeState>>,
    gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    config: Arc<ConfigMemory>,
    requests: mpsc::Receiver<SetModeRequest>,
    effective_mode: watch::Sender<Modes>,
}

impl DisplayModeTask {
    pub fn new(
        mode: ModeInterface,
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        runtime: ModeRuntime,
    ) -> Self {
        Self {
            mode,
            mode_state,
            gpu_list,
            config,
            requests: runtime.requests,
            effective_mode: runtime.effective_mode,
        }
    }

    fn external_display_target(requested: Modes, connected: bool, enabled: bool) -> Modes {
        if enabled && connected && matches!(requested, Modes::Integrated | Modes::Smart) {
            Modes::Hybrid
        } else {
            requested
        }
    }

    fn auto_switch_enabled(&self) -> bool {
        self.config
            .external_display_auto_switch
            .load(Ordering::Relaxed)
    }

    async fn desired_mode(&self) -> Modes {
        self.mode_state.read().await.mode()
    }

    async fn monitor_enabled(&self) -> bool {
        self.auto_switch_enabled() && self.desired_mode().await != Modes::Manual
    }

    async fn required_external_card(&self) -> fdo::Result<Option<u32>> {
        let card = {
            let gpu_list = self.gpu_list.read().await;
            if gpu_list.len() != 2 {
                return Ok(None);
            }

            let mut default_count = 0;
            let mut non_default_card = None;
            for gpu in gpu_list.values() {
                if gpu.device.is_default() {
                    default_count += 1;
                } else if non_default_card.replace(*gpu.device.card()).is_some() {
                    return Ok(None);
                }
            }
            if default_count != 1 {
                return Ok(None);
            }
            non_default_card
        };

        match card {
            Some(card)
                if external_display_connected(card).map_err(|err| {
                    fdo::Error::Failed(format!("failed to read DRM connector state: {err}"))
                })? =>
            {
                Ok(Some(card))
            }
            _ => Ok(None),
        }
    }

    async fn required_external_display_connected(&self) -> fdo::Result<bool> {
        self.required_external_card()
            .await
            .map(|card| card.is_some())
    }

    async fn notify_drm_change(card: u32) {
        let path = format!("/sys/class/drm/card{card}/uevent");
        if let Err(err) = tokio::fs::write(&path, "change\n").await {
            warn!("failed to replay DRM change event through {path}: {err}");
        }
    }

    async fn select_target(&self, requested: Modes) -> fdo::Result<(Modes, Option<u32>)> {
        let enabled = self.auto_switch_enabled();
        let card = if enabled && matches!(requested, Modes::Integrated | Modes::Smart) {
            self.required_external_card().await?
        } else {
            None
        };
        Ok((
            Self::external_display_target(requested, card.is_some(), enabled),
            card,
        ))
    }

    async fn apply_effective_mode(&self, mode: Modes, force_apply: bool) -> fdo::Result<bool> {
        let previous_mode = *self.effective_mode.borrow();
        if force_apply || mode != previous_mode {
            if let Err(err) = self.mode.apply_mode(mode).await {
                if let Err(rollback_err) = self.mode.apply_mode(previous_mode).await {
                    warn!(
                        "failed to restore previous mode ({previous_mode}) after apply_mode error: {rollback_err}"
                    );
                }
                return Err(err);
            }
            self.effective_mode.send_replace(mode);
        }
        Ok(mode != previous_mode)
    }

    async fn apply_requested_mode(&self, requested: Modes, force_apply: bool) -> fdo::Result<bool> {
        let (target, card) = self.select_target(requested).await?;
        let changed = self.apply_effective_mode(target, force_apply).await?;
        if changed
            && target == Modes::Hybrid
            && requested != Modes::Hybrid
            && let Some(card) = card
        {
            Self::notify_drm_change(card).await;
        }
        Ok(changed)
    }

    async fn save_desired_mode(&self, mode: Modes) {
        let mut state = self.mode_state.write().await;
        if state.mode() != mode
            && let Err(err) = state.save_state(mode).await
        {
            warn!("mode couldn't be saved to state: {err}");
        }
    }

    async fn set_requested_mode(&self, mode: Modes) -> fdo::Result<bool> {
        let changed = self.apply_requested_mode(mode, false).await?;
        self.save_desired_mode(mode).await;
        Ok(changed)
    }

    async fn reconcile_external_display(&self) -> fdo::Result<bool> {
        let requested = self.desired_mode().await;
        self.apply_requested_mode(requested, false).await
    }

    async fn emit_mode_change(&self, interface: &InterfaceRef<ModeInterface>, changed: bool) {
        if let Err(err) = self.mode.emit_mode_change(interface, changed).await {
            error!("failed to emit automatic mode change: {err}");
        }
    }

    pub async fn apply_at_startup(&self) -> fdo::Result<()> {
        let requested = self.desired_mode().await;
        let (target, card) = match self.select_target(requested).await {
            Ok(target) => target,
            Err(err) => {
                warn!("failed to read external display topology at startup: {err}");
                (requested, None)
            }
        };
        let changed = self.apply_effective_mode(target, true).await?;
        if changed
            && target == Modes::Hybrid
            && requested != Modes::Hybrid
            && let Some(card) = card
        {
            Self::notify_drm_change(card).await;
        }
        Ok(())
    }

    pub async fn apply_startup_fallback(&self) -> fdo::Result<()> {
        self.apply_effective_mode(Modes::Manual, true).await?;
        self.save_desired_mode(Modes::Manual).await;
        Ok(())
    }

    async fn handle_request(&self, request: SetModeRequest) -> bool {
        let result = self.set_requested_mode(request.requested).await.map(|_| ());
        let succeeded = result.is_ok();
        let _ = request.response.send(result);
        succeeded
    }

    async fn observe_topology(
        &self,
        interface: &InterfaceRef<ModeInterface>,
        state: &mut DisplayMonitorState,
        topology_event: bool,
    ) {
        let enabled = self.monitor_enabled().await;
        let connected = if enabled {
            match self.required_external_display_connected().await {
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
            match self.reconcile_external_display().await {
                Ok(changed) => self.emit_mode_change(interface, changed).await,
                Err(err) => warn!("failed to apply automatic external-display mode: {err}"),
            }
        }
    }

    pub async fn run(mut self, mode_interface: InterfaceRef<ModeInterface>) -> zbus::Result<()> {
        let (topology_events, mut topology_rx) = mpsc::channel(1);
        let topology_guard = topology_events.clone();
        let watcher = task::spawn(watch_drm_events(topology_events));
        let mut retry = tokio::time::interval(RETRY_INTERVAL);
        retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut state = DisplayMonitorState::default();

        self.observe_topology(&mode_interface, &mut state, false)
            .await;
        retry.tick().await;

        loop {
            let debounce_deadline = state.debounce_deadline.unwrap_or_else(Instant::now);
            let restore_deadline = state.restore_deadline.unwrap_or_else(Instant::now);
            tokio::select! {
                request = self.requests.recv() => {
                    let Some(request) = request else {
                        break;
                    };
                    if self.handle_request(request).await {
                        state.restore_deadline = None;
                    }
                }
                event = topology_rx.recv() => {
                    if event.is_some() {
                        state.debounce(Instant::now());
                    }
                }
                _ = retry.tick() => {
                    self.observe_topology(&mode_interface, &mut state, false).await;
                }
                _ = tokio::time::sleep_until(debounce_deadline), if state.debounce_deadline.is_some() => {
                    state.debounce_deadline = None;
                    self.observe_topology(&mode_interface, &mut state, true).await;
                }
                _ = tokio::time::sleep_until(restore_deadline), if state.restore_deadline.is_some() => {
                    state.restore_deadline = None;
                    self.observe_topology(&mode_interface, &mut state, false).await;
                }
            }
        }

        drop(topology_guard);
        watcher.abort();
        Ok(())
    }
}

async fn run_drm_event_watcher(events: &mpsc::Sender<()>) -> zbus::Result<()> {
    let drm_monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?;
    let drm_fd = AsyncFd::new(drm_monitor.listen()?)?;

    loop {
        let mut guard = drm_fd.ready(Interest::READABLE).await?;
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
            let _ = events.try_send(());
        }
    }
}

async fn watch_drm_events(events: mpsc::Sender<()>) {
    loop {
        if events.is_closed() {
            return;
        }
        if let Err(err) = run_drm_event_watcher(&events).await {
            warn!(
                "display event watcher exited with error: {err}; retrying in {} seconds",
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
                DisplayModeTask::external_display_target(mode, true, true),
                Modes::Hybrid
            );
        }
        for mode in [Modes::Hybrid, Modes::Manual] {
            assert_eq!(
                DisplayModeTask::external_display_target(mode, true, true),
                mode
            );
        }
        for mode in [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ] {
            assert_eq!(
                DisplayModeTask::external_display_target(mode, true, false),
                mode
            );
            assert_eq!(
                DisplayModeTask::external_display_target(mode, false, true),
                mode
            );
        }
    }

    #[test]
    fn topology_events_are_debounced() {
        let start = Instant::now();
        let mut state = DisplayMonitorState::default();
        state.debounce(start);
        let first = state.debounce_deadline.unwrap();
        state.debounce(start + Duration::from_millis(100));
        assert!(state.debounce_deadline.unwrap() > first);
    }

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
