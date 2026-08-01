//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use std::sync::{
    Arc, atomic::{AtomicBool, AtomicU32, Ordering}
};

use log::info;
use tokio_stream::StreamExt;
use zbus::{Connection, Result, object_server::InterfaceRef, proxy};

use crate::interface::{ModeInterface, Modes};

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    #[zbus(property)]
    fn on_battery(&self) -> Result<bool>;
}
pub async fn watch_battery_status(
    switch_setting: Arc<AtomicBool>,
    switch_mode: Arc<AtomicU32>,
    mode: ModeInterface,
    mode_interface: InterfaceRef<ModeInterface>,
) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let upower_proxy = UPowerProxy::new(&connection).await?;

    let mut battery_stream = upower_proxy.receive_on_battery_changed().await;
    // only when setting is enabled
    while let Some(msg) = battery_stream.next().await {
        if !switch_setting.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }
        if let Ok(state) = msg.get().await {
            info!("battery event detected: {:?}", state);
            // now get the configured mode and change
            let target = if state {
                Modes::Integrated
            } else {
                match Modes::try_from(switch_mode.load(Ordering::Relaxed)) {
                    Ok(mode) => mode,
                    Err(_) => continue,
                }
            };
            // ignore dbus api error, it might happen on system with multiple gpus trying to switch
            // to hybrid, the daemon will just refuse
            if let Ok(changed) = mode.set_battery_mode_value(target).await {
                mode.emit_mode_change(&mode_interface, changed).await?;
            }
        }
    }

    Ok(())
}
