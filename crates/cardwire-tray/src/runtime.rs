use std::{collections::BTreeMap, time::Duration};

use ksni::TrayMethods;
use log::{info, warn};
use tokio::{
    sync::mpsc, task::JoinHandle, time::{MissedTickBehavior, interval, sleep}
};
use tokio_stream::StreamExt;

use crate::{
    applet::{CardwireTray, GpuInfo, TrayAction}, config::{TrayConfig, TrayMode}, dbus::CardwireClient
};

enum RuntimeEvent {
    Mode(TrayMode),
    PowerState(u32, String),
    BlockState(u32, bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiCommand {
    Open,
    Quit,
    Unavailable(String),
}

pub async fn run(
    gui_command_tx: mpsc::UnboundedSender<GuiCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (action_tx, mut action_rx) = mpsc::unbounded_channel();
    let tray_handle = CardwireTray::offline(action_tx).spawn().await?;

    // Each outer iteration owns one daemon connection and its signal watchers.
    // Losing the daemon returns here, marks the tray offline, and reconnects.
    'runtime: loop {
        let client = loop {
            if tray_handle.is_closed() {
                return Err(std::io::Error::other("status notifier service stopped").into());
            }
            match CardwireClient::connect().await {
                Ok(client) => break client,
                Err(error) => {
                    warn!("Cardwire daemon unavailable: {error}");
                    set_offline(&tray_handle).await;
                    // Tray actions must remain responsive while cardwired is down;
                    // in particular, users must still be able to open or quit.
                    tokio::select! {
                        () = sleep(Duration::from_secs(5)) => {}
                        action = action_rx.recv() => {
                            if process_offline_action(action, &gui_command_tx).await {
                                break 'runtime;
                            }
                        }
                    }
                }
            }
        };

        info!("Connected to the Cardwire daemon");
        let Ok((mut mode, mut gpus)) = client.snapshot().await else {
            set_offline(&tray_handle).await;
            sleep(Duration::from_secs(5)).await;
            continue;
        };
        update_snapshot(&tray_handle, mode, gpus.clone()).await;

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mut watchers = spawn_watchers(&client, &gpus, event_tx.clone()).await;
        let mut health = interval(Duration::from_secs(5));
        health.set_missed_tick_behavior(MissedTickBehavior::Skip);
        health.tick().await;
        let mut discovery = interval(Duration::from_secs(30));
        discovery.set_missed_tick_behavior(MissedTickBehavior::Skip);
        discovery.tick().await;

        let reconnect = loop {
            tokio::select! {
                action = action_rx.recv() => {
                    let Some(action) = action else { break false };
                    if action == TrayAction::Quit {
                        let _ = gui_command_tx.send(GuiCommand::Quit);
                        break false;
                    }
                    process_action(
                        action,
                        &client,
                        &tray_handle,
                        &gui_command_tx,
                        &mut mode,
                        &mut gpus,
                    ).await;
                }
                event = event_rx.recv() => {
                    if let Some(event) = event {
                        apply_event(event, &tray_handle, &mut mode, &mut gpus).await;
                    }
                }
                _ = health.tick() => {
                    if tray_handle.is_closed() {
                        return Err(std::io::Error::other("status notifier service stopped").into());
                    }
                    if client.status().await.is_err() {
                        break true;
                    }
                }
                _ = discovery.tick() => {
                    match client.gpus().await {
                        Ok(updated) if updated.keys().ne(gpus.keys()) => {
                            // Signal streams are bound to concrete GPU object paths.
                            // Rebuild all GPU watchers whenever topology changes.
                            abort_watchers(&mut watchers);
                            gpus = updated;
                            watchers = spawn_watchers(&client, &gpus, event_tx.clone()).await;
                            update_snapshot(&tray_handle, mode, gpus.clone()).await;
                        }
                        Ok(updated) => {
                            gpus = updated;
                            update_snapshot(&tray_handle, mode, gpus.clone()).await;
                        }
                        Err(_) => break true,
                    }
                }
            }
        };
        abort_watchers(&mut watchers);
        if !reconnect {
            break;
        }
        set_offline(&tray_handle).await;
    }

    tray_handle.shutdown().await;
    Ok(())
}

async fn process_offline_action(
    action: Option<TrayAction>,
    gui_command_tx: &mpsc::UnboundedSender<GuiCommand>,
) -> bool {
    match action {
        Some(TrayAction::OpenGui) => {
            let _ = gui_command_tx.send(GuiCommand::Open);
        }
        Some(
            TrayAction::ToggleConfiguredMode
            | TrayAction::SetMode(_)
            | TrayAction::SetGpuBlock { .. },
        ) => {
            notify("Cardwire daemon is unavailable").await;
        }
        Some(TrayAction::Quit) => {
            let _ = gui_command_tx.send(GuiCommand::Quit);
            return true;
        }
        None => return true,
    }
    false
}

async fn process_action(
    action: TrayAction,
    client: &CardwireClient,
    tray_handle: &ksni::Handle<CardwireTray>,
    gui_command_tx: &mpsc::UnboundedSender<GuiCommand>,
    mode: &mut TrayMode,
    gpus: &mut BTreeMap<u32, GpuInfo>,
) {
    match action {
        TrayAction::ToggleConfiguredMode => {
            let config = match TrayConfig::load() {
                Ok(config) => config,
                Err(error) => {
                    warn!("Invalid tray configuration: {error}");
                    notify("Invalid tray settings; using Integrated ↔ Hybrid").await;
                    TrayConfig::default()
                }
            };
            set_mode(client, tray_handle, mode, gpus, config.next_mode(*mode)).await;
        }
        TrayAction::SetMode(new_mode) => {
            set_mode(client, tray_handle, mode, gpus, new_mode).await;
        }
        TrayAction::SetGpuBlock { id, blocked } => match client.set_gpu_block(id, blocked).await {
            Ok(()) => {
                if let Some(gpu) = gpus.get_mut(&id) {
                    gpu.blocked = blocked;
                }
                update_snapshot(tray_handle, *mode, gpus.clone()).await;
                notify(&format!(
                    "GPU {id} {}",
                    if blocked { "blocked" } else { "unblocked" }
                ))
                .await;
            }
            Err(error) => notify(&format!("Could not update GPU {id}: {error}")).await,
        },
        TrayAction::OpenGui => {
            let _ = gui_command_tx.send(GuiCommand::Open);
        }
        TrayAction::Quit => {}
    }
}

async fn set_mode(
    client: &CardwireClient,
    tray_handle: &ksni::Handle<CardwireTray>,
    current_mode: &mut TrayMode,
    gpus: &mut BTreeMap<u32, GpuInfo>,
    new_mode: TrayMode,
) {
    match client.set_mode(new_mode).await {
        Ok(()) => {
            *current_mode = new_mode;
            if let Ok(updated) = client.gpus().await {
                *gpus = updated;
            }
            update_snapshot(tray_handle, *current_mode, gpus.clone()).await;
            notify(&format!("Switched to {new_mode} mode")).await;
        }
        Err(error) => notify(&format!("Could not switch to {new_mode} mode: {error}")).await,
    }
}

async fn spawn_watchers(
    client: &CardwireClient,
    gpus: &BTreeMap<u32, GpuInfo>,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
) -> Vec<JoinHandle<()>> {
    // Keep zbus signal streams task-local and forward small owned events to the
    // state machine. This avoids borrowing proxies across the main select loop.
    let mut handles = Vec::new();
    let mode_client = client.clone();
    let mode_tx = event_tx.clone();
    handles.push(tokio::spawn(async move {
        let Ok(proxy) = mode_client.mode_proxy().await else {
            return;
        };
        let mut changes = proxy.receive_mode_changed().await;
        while let Some(change) = changes.next().await {
            if let Ok(value) = change.get().await
                && let Some(mode) = TrayMode::from_value(value)
                && mode_tx.send(RuntimeEvent::Mode(mode)).is_err()
            {
                break;
            }
        }
    }));

    for id in gpus.keys().copied() {
        let gpu_client = client.clone();
        let gpu_tx = event_tx.clone();
        handles.push(tokio::spawn(async move {
            let Ok(proxy) = gpu_client.gpu_proxy(id).await else {
                return;
            };
            let Ok(mut power_changes) = proxy.receive_power_state_changed().await else {
                return;
            };
            let mut block_changes = proxy.receive_block_changed().await;
            loop {
                tokio::select! {
                    change = power_changes.next() => {
                        let Some(change) = change else { break };
                        if let Ok(arguments) = change.args()
                            && gpu_tx.send(RuntimeEvent::PowerState(id, arguments.state)).is_err()
                        {
                            break;
                        }
                    }
                    change = block_changes.next() => {
                        let Some(change) = change else { break };
                        if let Ok(blocked) = change.get().await
                            && gpu_tx.send(RuntimeEvent::BlockState(id, blocked)).is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }));
    }
    handles
}

fn abort_watchers(handles: &mut Vec<JoinHandle<()>>) {
    for handle in handles.drain(..) {
        handle.abort();
    }
}

async fn apply_event(
    event: RuntimeEvent,
    tray_handle: &ksni::Handle<CardwireTray>,
    mode: &mut TrayMode,
    gpus: &mut BTreeMap<u32, GpuInfo>,
) {
    match event {
        RuntimeEvent::Mode(new_mode) => *mode = new_mode,
        RuntimeEvent::PowerState(id, state) => {
            if let Some(gpu) = gpus.get_mut(&id) {
                gpu.power_state = state;
            }
        }
        RuntimeEvent::BlockState(id, blocked) => {
            if let Some(gpu) = gpus.get_mut(&id) {
                gpu.blocked = blocked;
            }
        }
    }
    update_snapshot(tray_handle, *mode, gpus.clone()).await;
}

async fn update_snapshot(
    tray_handle: &ksni::Handle<CardwireTray>,
    mode: TrayMode,
    gpus: BTreeMap<u32, GpuInfo>,
) {
    let _ = tray_handle
        .update(|tray| {
            tray.online = true;
            tray.mode = Some(mode);
            tray.gpus = gpus;
        })
        .await;
}

async fn set_offline(tray_handle: &ksni::Handle<CardwireTray>) {
    let _ = tray_handle
        .update(|tray| {
            tray.online = false;
            tray.mode = None;
            tray.gpus.clear();
        })
        .await;
}

async fn notify(message: &str) {
    let message = message.to_string();
    // notify-rust exposes a blocking API; keep it off the async runtime workers.
    let _ = tokio::task::spawn_blocking(move || {
        notify_rust::Notification::new()
            .summary("Cardwire")
            .body(&message)
            .icon("com.github.opengamingcollective.cardwire.tray")
            .show()
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_action_requests_the_hosted_gui_window() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        assert!(!process_offline_action(Some(TrayAction::OpenGui), &sender).await);
        assert_eq!(receiver.try_recv().unwrap(), GuiCommand::Open);
    }

    #[tokio::test]
    async fn quit_action_stops_tray_and_combined_gui() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        assert!(process_offline_action(Some(TrayAction::Quit), &sender).await);
        assert_eq!(receiver.try_recv().unwrap(), GuiCommand::Quit);
    }
}
