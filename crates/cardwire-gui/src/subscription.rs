use std::collections::{BTreeMap, HashMap};

use iced::{
    Subscription, futures::{SinkExt, StreamExt, channel::mpsc::Sender}, stream
};

use log::{error, warn};
use tokio::select;
use tokio_stream::StreamMap;

use crate::{
    helpers::CardwireDbus, message::Message, models::{DaemonSettings, Mode, PciDevice}
};
use zbus::{
    Connection, Proxy, names::OwnedInterfaceName, proxy, zvariant::{OwnedObjectPath, OwnedValue}
};

// CardwireMode is used to listen to mode change signals

#[proxy(
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire",
    interface = "com.github.opengamingcollective.cardwire.Mode"
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
                    return;
                }
            };

            let proxy = match CardwireModeProxy::new(&connection).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create D-Bus proxy: {}", e);
                    return;
                }
            };
            // for startup
            if let Ok(initial_mode) = proxy.mode().await {
                let mode_into_enum = Mode::from_repr(initial_mode);
                if let Some(mode) = mode_into_enum {
                    let _ = output.send(Message::FetchedMode(Ok(mode))).await;
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
        })
    })
}

// CardwireSetting is used to listen to config changes signal
#[proxy(
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire",
    interface = "com.github.opengamingcollective.cardwire.Config"
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
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire",
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
            if let Ok(gpu_list) = CardwireDbus::new().get_devices_list().await {
                let _ = output.send(Message::AllDevicesFetched(Ok(gpu_list))).await;
            };

            let proxy = match CardwireGpuIntProxy::new(&connection).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to create D-Bus proxy: {}", e);
                    return;
                }
            };
            // mutable so it can be update later if list refresh
            #[allow(unused_mut)]
            let mut gpu_objects = match proxy.get_managed_objects().await {
                Ok(list) => list,
                Err(e) => {
                    warn!("Failed to retrieve dbus managed objects (gpu_list): {}", e);
                    return;
                }
            };

            // Count the number of gpus, if the daemon didn't mess the BTree, having count = 2 means
            // there is a gpu at 0 and at 1
            let mut dbus_streams = StreamMap::new();
            let mut dbus_properties = StreamMap::new();
            for (path, _) in gpu_objects {
                let path_str = path.as_str();
                if let Some(id_str) =
                    path_str.strip_prefix("/com/github/opengamingcollective/cardwire/Gpu/")
                    && let Ok(id) = id_str.parse::<u32>()
                {
                    let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
                    let gpu_proxy = match Proxy::new(
                        &connection,
                        "com.github.opengamingcollective.cardwire",
                        path,
                        "com.github.opengamingcollective.cardwire.Gpu",
                    )
                    .await
                    {
                        Ok(p) => p,
                        Err(e) => {
                            error!("Couldn't create gpu {} proxy: {}", id, e);
                            return;
                        }
                    };
                    // First time we need to populate the gpu power_state
                    if let Ok(power_state) =
                        gpu_proxy.call::<&str, (), String>("PowerState", &()).await
                    {
                        let _ = output
                            .send(Message::UpdateGpuPowerState(id as usize, power_state))
                            .await;
                    }

                    let power_signal = match gpu_proxy.receive_signal("PowerStateChanged").await {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Couldn't receive gpu {} power signal: {}", id, e);
                            return;
                        }
                    };
                    let stream_name = format!("gpu_power_{}", id);
                    dbus_streams.insert(stream_name, power_signal);

                    let block_signal: proxy::PropertyStream<'_, bool> =
                        gpu_proxy.receive_property_changed("Block").await;

                    let stream_name = format!("gpu_block_{}", id);
                    dbus_properties.insert(stream_name, block_signal);
                }
            }
            loop {
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
                }
            }
        })
    })
}

#[proxy(
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire",
    interface = "com.github.opengamingcollective.cardwire.Debug"
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

pub fn dbus_sub() -> Subscription<Message> {
    Subscription::batch([config_sub(), mode_sub(), gpu_sub(), pci_sub()])
}
