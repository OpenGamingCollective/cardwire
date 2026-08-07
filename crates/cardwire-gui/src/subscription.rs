use std::collections::{BTreeMap, HashMap, VecDeque};

use iced::{
    Subscription, futures::{SinkExt, StreamExt, channel::mpsc::Sender}, stream
};

use log::{info, warn};
use tokio::select;
use tokio_stream::StreamMap;

use crate::{
    helpers::CardwireDbus, message::Message, models::{DaemonSettings, LogEntry, Mode, PciDevice}, tray
};
use zbus::{
    Connection, Proxy, names::OwnedInterfaceName, proxy, zvariant::{OwnedObjectPath, OwnedValue}
};

pub fn tray_sub() -> Subscription<Message> {
    Subscription::run_with("cardwire_tray_subscription", |_id| {
        stream::channel(10, |mut output: Sender<Message>| async move {
            let (handle, mut actions) = match tray::spawn().await {
                Ok(tray) => tray,
                Err(error) => {
                    let _ = output
                        .send(Message::TrayUnavailable(error.to_string()))
                        .await;
                    std::future::pending::<()>().await;
                    return;
                }
            };

            if output.send(Message::TrayReady(handle)).await.is_err() {
                return;
            }
            while let Some(action) = actions.recv().await {
                let quitting = action == tray::TrayAction::Quit;
                if output.send(Message::TrayAction(action)).await.is_err() {
                    return;
                }
                if quitting {
                    std::future::pending::<()>().await;
                    return;
                }
            }

            let _ = output
                .send(Message::TrayUnavailable(
                    "tray applet stopped unexpectedly".to_string(),
                ))
                .await;
            std::future::pending::<()>().await;
        })
    })
}

// CardwireMode is used to listen to mode change signals

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Mode"
)]
// org.freedesktop.DBus.Properties
trait CardwireMode {
    #[zbus(property)]
    fn mode(&self) -> zbus::Result<u32>;
}
fn mode_sub() -> Subscription<Message> {
    Subscription::run_with("cardwire_mode_subscription", |_id| {
        stream::channel(100, |mut output: Sender<Message>| async move {
            let connection = match Connection::system().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("Failed to connect to D-Bus: {}", e);
                    let _ = output.send(Message::FetchedMode(Err(e.to_string()))).await;
                    return;
                }
            };

            let proxy = match CardwireModeProxy::new(&connection).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create D-Bus proxy: {}", e);
                    let _ = output.send(Message::FetchedMode(Err(e.to_string()))).await;
                    return;
                }
            };
            // for startup
            match proxy.mode().await {
                Ok(initial_mode) => {
                    if let Some(mode) = Mode::from_repr(initial_mode) {
                        let _ = output.send(Message::FetchedMode(Ok(mode))).await;
                    } else {
                        let _ = output
                            .send(Message::FetchedMode(Err(format!(
                                "Unknown mode: {initial_mode}"
                            ))))
                            .await;
                    }
                }
                Err(error) => {
                    let _ = output
                        .send(Message::FetchedMode(Err(error.to_string())))
                        .await;
                }
            }
            let mut mode_stream = proxy.receive_mode_changed().await;
            while let Some(change) = mode_stream.next().await {
                if let Ok(new_mode) = change.get().await {
                    let mode_into_enum = Mode::from_repr(new_mode);

                    if let Some(mode) = mode_into_enum {
                        let _ = output.send(Message::FetchedMode(Ok(mode))).await;
                    }
                }
            }
            let _ = output
                .send(Message::FetchedMode(Err(
                    "Cardwire daemon disconnected".to_string()
                )))
                .await;
        })
    })
}

// CardwireSetting is used to listen to config changes signal
#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Config"
)]
trait CardwireConfig {
    #[zbus(property)]
    fn experimental_nvidia_block(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn auto_apply_gpu_state(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn battery_auto_switch(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn battery_auto_switch_mode(&self) -> zbus::Result<u32>;
}

fn config_sub() -> Subscription<Message> {
    Subscription::run_with("cardwire_config_subscription", |_id| {
        stream::channel(100, |mut output: Sender<Message>| async move {
            let connection = match Connection::system().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("Failed to connect to D-Bus: {}", e);
                    return;
                }
            };

            let proxy = match CardwireConfigProxy::new(&connection).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create D-Bus proxy: {}", e);
                    return;
                }
            };
            let mut config_nvidia_stream = proxy.receive_experimental_nvidia_block_changed().await;
            let mut config_auto_apply_stream = proxy.receive_auto_apply_gpu_state_changed().await;
            let mut config_switch_battery = proxy.receive_battery_auto_switch_changed().await;
            let mut config_switch_battery_mode =
                proxy.receive_battery_auto_switch_mode_changed().await;

            // Fetch the initial values once so the toggles render the real daemon state on
            // startup, before any change signal arrives
            match proxy.experimental_nvidia_block().await {
                Ok(state) => {
                    let _ = output
                        .send(Message::FetchedSetting(Ok((
                            DaemonSettings::ExpNvidiaBlock,
                            Some(state),
                            None,
                        ))))
                        .await;
                }
                Err(e) => warn!("Failed to fetch experimental_nvidia_block: {}", e),
            }
            match proxy.auto_apply_gpu_state().await {
                Ok(state) => {
                    let _ = output
                        .send(Message::FetchedSetting(Ok((
                            DaemonSettings::AutoApplyGpuState,
                            Some(state),
                            None,
                        ))))
                        .await;
                }
                Err(e) => warn!("Failed to fetch auto_apply_gpu_state: {}", e),
            }
            match proxy.battery_auto_switch().await {
                Ok(state) => {
                    let _ = output
                        .send(Message::FetchedSetting(Ok((
                            DaemonSettings::BattAutoSwitch,
                            Some(state),
                            None,
                        ))))
                        .await;
                }
                Err(e) => warn!("Failed to fetch battery_auto_switch: {}", e),
            }
            match proxy.battery_auto_switch_mode().await {
                Ok(mode) => {
                    if let Some(mode) = Mode::from_repr(mode) {
                        let _ = output
                            .send(Message::FetchedSetting(Ok((
                                DaemonSettings::BattAutoSwitchMode,
                                None,
                                Some(mode),
                            ))))
                            .await;
                    }
                }
                Err(e) => warn!("Failed to fetch battery_auto_switch_mode: {}", e),
            }

            loop {
                select! {
                    // Exp nvidia block
                    Some(change) = config_nvidia_stream.next() => {
                        if let Ok(new_state) = change.get().await {
                            let _ = output.send(Message::FetchedSetting(Ok((
                                DaemonSettings::ExpNvidiaBlock,
                                Some(new_state),
                                None,
                            )))).await;
                        }
                    },
                    // Auto Apply GPU State
                    Some(change) = config_auto_apply_stream.next() => {
                        if let Ok(new_state) = change.get().await {
                            let _ = output.send(Message::FetchedSetting(Ok((
                                DaemonSettings::AutoApplyGpuState,
                                Some(new_state),
                                None,
                            )))).await;
                        }
                    },
                    // Auto Switch on battery
                    Some(change) = config_switch_battery.next() => {
                        if let Ok(new_state) = change.get().await {
                            let _ = output.send(Message::FetchedSetting(Ok((
                                DaemonSettings::BattAutoSwitch,
                                Some(new_state),
                                None,
                            )))).await;
                        }
                    },
                    // Auto Switch Mode
                    Some(change) = config_switch_battery_mode.next() => {
                        if let Ok(new_mode) = change.get().await {
                            let mode_into_enum = Mode::from_repr(new_mode);
                            if let Some(mode) = mode_into_enum {
                             let _ = output.send(Message::FetchedSetting(Ok((
                                    DaemonSettings::BattAutoSwitchMode,
                                    None,
                                    Some(mode),
                                )))).await;
                            }
                        }
                    },
                }
            }
        })
    })
}

// This one is to listen to dbus gpu's object
// daemon doesn't send signal on gpu_refresh event, will be implemented later
// for now this one implement power_state and block
#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.freedesktop.DBus.ObjectManager"
)]
trait CardwireGpuInt {
    #[allow(clippy::type_complexity)]
    fn get_managed_objects(
        &self,
    ) -> zbus::Result<
        HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>,
    >;
}
fn gpu_sub() -> Subscription<Message> {
    Subscription::run_with("cardwire_gpu_subscription", |_| {
        stream::channel(100, |mut output: Sender<Message>| async move {
            let connection = match Connection::system().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("Failed to connect to D-Bus: {}", e);
                    return;
                }
            };

            // First populate the daemon gpu_list
            match CardwireDbus::new().get_devices_list().await {
                Ok(gpu_list) => {
                    let _ = output.send(Message::AllDevicesFetched(Ok(gpu_list))).await;
                }
                Err(error) => {
                    let _ = output
                        .send(Message::AllDevicesFetched(Err(error.to_string())))
                        .await;
                }
            }

            // Listen to the daemon's ObjectManager so GPU hotplug or a daemon-side refresh is
            // picked up
            let om_proxy = match Proxy::new(
                &connection,
                "org.opengamingcollective.cardwire",
                "/org/opengamingcollective/cardwire",
                "org.freedesktop.DBus.ObjectManager",
            )
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create ObjectManager proxy: {}", e);
                    return;
                }
            };
            let mut om_added = match om_proxy.receive_signal("InterfacesAdded").await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to subscribe to InterfacesAdded: {}", e);
                    return;
                }
            };
            let mut om_removed = match om_proxy.receive_signal("InterfacesRemoved").await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to subscribe to InterfacesRemoved: {}", e);
                    return;
                }
            };

            let mut dbus_streams: StreamMap<String, proxy::SignalStream<'static>> =
                StreamMap::new();
            let mut dbus_properties: StreamMap<String, proxy::PropertyStream<'static, bool>> =
                StreamMap::new();
            if let Err(e) = build_gpu_streams(
                &connection,
                &mut output,
                &mut dbus_streams,
                &mut dbus_properties,
            )
            .await
            {
                warn!("Failed to retrieve dbus managed objects (gpu_list): {}", e);
                return;
            }

            loop {
                let mut needs_refresh = false;
                select! {
                    Some(msg) = dbus_streams.next() => {
                        let msg_id = msg.0;
                        let msg = msg.1;
                        match msg_id {
                            // gpu_power state
                            _ if msg_id.starts_with("gpu_power") => {
                                // Turn the body into a string and parse the gpu id
                                if let Ok(power_state) = msg.body().deserialize::<String>()
                                    && let Some(id_str) = msg_id.strip_prefix("gpu_power_")
                                    && let Ok(id) = id_str.parse::<usize>()
                                {
                                    let _ = output
                                        .send(Message::UpdateGpuPowerState(id, power_state))
                                        .await;
                                }
                            }
                            _ => {}
                        }
                    },
                    Some(msg) = dbus_properties.next() => {
                        let msg_id = msg.0;
                        let msg = msg.1;
                        match msg_id {
                            _ if msg_id.starts_with("gpu_block") => {
                                let zbus_response = msg.get().await;
                                if let Ok(new_state) = zbus_response &&
                                    let Some(id_str) = msg_id.strip_prefix("gpu_block_") &&
                                        let Ok(id) = id_str.parse::<usize>(){
                                    let _ = output.send(Message::UpdateBlockState(id, new_state)).await;
                                }
                            },
                            _ => {}
                        }
                    },
                    Some(msg) = om_added.next() => {
                        if let Ok((path, _)) = msg.body().deserialize::<(
                            OwnedObjectPath,
                            HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>,
                        )>()
                            && path.as_str().starts_with("/org/opengamingcollective/cardwire/Gpu/")
                        {
                            needs_refresh = true;
                        }
                    },
                    Some(msg) = om_removed.next() => {
                        if let Ok((path, _)) = msg.body().deserialize::<(
                            OwnedObjectPath,
                            Vec<OwnedInterfaceName>,
                        )>()
                            && path.as_str().starts_with("/org/opengamingcollective/cardwire/Gpu/")
                        {
                            needs_refresh = true;
                        }
                    },
                }
                if needs_refresh {
                    info!("GPU list changed on the daemon, refreshing");
                    // Refetch the gpu list and rebuild the signal streams from the new GPU set
                    match CardwireDbus::new().get_devices_list().await {
                        Ok(gpu_list) => {
                            let _ = output.send(Message::AllDevicesFetched(Ok(gpu_list))).await;
                        }
                        Err(error) => {
                            let _ = output
                                .send(Message::AllDevicesFetched(Err(error.to_string())))
                                .await;
                        }
                    }
                    dbus_streams.clear();
                    dbus_properties.clear();
                    if let Err(e) = build_gpu_streams(
                        &connection,
                        &mut output,
                        &mut dbus_streams,
                        &mut dbus_properties,
                    )
                    .await
                    {
                        warn!("Failed to rebuild GPU signal streams: {}", e);
                    }
                }
            }
        })
    })
}

/// Fetch the current GPU set from the daemon and (re)build the per-GPU signal streams
/// (PowerStateChanged and Block property changes)
async fn build_gpu_streams(
    connection: &Connection,
    output: &mut Sender<Message>,
    dbus_streams: &mut StreamMap<String, proxy::SignalStream<'static>>,
    dbus_properties: &mut StreamMap<String, proxy::PropertyStream<'static, bool>>,
) -> zbus::Result<()> {
    let proxy = CardwireGpuIntProxy::new(connection).await?;
    let gpu_objects = proxy.get_managed_objects().await?;

    for (path, _) in gpu_objects {
        let path_str = path.as_str();
        if let Some(id_str) = path_str.strip_prefix("/org/opengamingcollective/cardwire/Gpu/")
            && let Ok(id) = id_str.parse::<u32>()
        {
            let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
            let gpu_proxy = Proxy::new(
                connection,
                "org.opengamingcollective.cardwire",
                path,
                "org.opengamingcollective.cardwire.Gpu",
            )
            .await?;
            // First time we need to populate the gpu power_state
            if let Ok(power_state) = gpu_proxy.call::<&str, (), String>("PowerState", &()).await {
                let _ = output
                    .send(Message::UpdateGpuPowerState(id as usize, power_state))
                    .await;
            }

            let power_signal = gpu_proxy.receive_signal("PowerStateChanged").await?;
            let stream_name = format!("gpu_power_{}", id);
            dbus_streams.insert(stream_name, power_signal);

            let block_signal: proxy::PropertyStream<'_, bool> =
                gpu_proxy.receive_property_changed("Block").await;

            let stream_name = format!("gpu_block_{}", id);
            dbus_properties.insert(stream_name, block_signal);
        }
    }
    Ok(())
}

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Debug"
)]
trait CardwireDebug {
    fn get_pci_devices(&self) -> zbus::Result<BTreeMap<String, PciDevice>>;
}
/// PCI Signal is not implemented yet, it just fetch the pci list at launch
fn pci_sub() -> Subscription<Message> {
    Subscription::run_with("cardwire_pci_subscription", |_| {
        stream::channel(100, |mut output: Sender<Message>| async move {
            let connection = match Connection::system().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("Failed to connect to D-Bus: {}", e);
                    return;
                }
            };

            let proxy = match CardwireDebugProxy::new(&connection).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create D-Bus proxy: {}", e);
                    return;
                }
            };

            // For now we only fetch the pci list at launch since the daemon doesnt implement a
            // signal
            if let Ok(pci_list) = proxy.get_pci_devices().await {
                let _ = output.send(Message::FetchedPciList(pci_list)).await;
            }
        })
    })
}

// CardwireLogger is used to listen to log signals

#[proxy(
    default_service = "org.opengamingcollective.cardwire",
    default_path = "/org/opengamingcollective/cardwire",
    interface = "org.opengamingcollective.cardwire.Logger"
)]
// org.freedesktop.DBus.Properties
trait CardwireLogger {
    fn process_blocked(&self) -> zbus::Result<VecDeque<LogEntry>>;
    #[zbus(signal)]
    fn process_blocked_changed(&self, log: LogEntry) -> zbus::Result<()>;
}

fn logger_sub() -> Subscription<Message> {
    Subscription::run_with("cardwire_logger_subscription", |_id| {
        stream::channel(100, |mut output: Sender<Message>| async move {
            let connection = match Connection::system().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("Failed to connect to D-Bus: {}", e);
                    let _ = output.send(Message::FetchedLogs(Err(e.to_string()))).await;
                    return;
                }
            };

            let proxy = match CardwireLoggerProxy::new(&connection).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create D-Bus proxy: {}", e);
                    let _ = output.send(Message::FetchedLogs(Err(e.to_string()))).await;
                    return;
                }
            };
            // for startup, get current blocked apps logs
            match proxy.process_blocked().await {
                Ok(initial_logs) => {
                    if !initial_logs.is_empty() {
                        let _ = output.send(Message::FetchedLogs(Ok(initial_logs))).await;
                    }
                }
                Err(error) => {
                    let _ = output
                        .send(Message::FetchedLogs(Err(error.to_string())))
                        .await;
                }
            }
            let mut logs_stream = match proxy.receive_process_blocked_changed().await {
                Ok(stream) => stream,
                Err(err) => {
                    warn!("Failed to subscribe to D-Bus logs signal: {}", err);
                    let _ = output
                        .send(Message::FetchedLogs(Err(err.to_string())))
                        .await;
                    return;
                }
            };
            while let Some(log_signal) = logs_stream.next().await {
                if let Ok(log_arg) = log_signal.args() {
                    let log: LogEntry = log_arg.log().clone();
                    let _ = output.send(Message::NewLog(log)).await;
                }
            }
            let _ = output
                .send(Message::FetchedMode(Err(
                    "Cardwire daemon disconnected".to_string()
                )))
                .await;
        })
    })
}

pub fn dbus_sub() -> Subscription<Message> {
    Subscription::batch([config_sub(), mode_sub(), gpu_sub(), pci_sub(), logger_sub()])
}
