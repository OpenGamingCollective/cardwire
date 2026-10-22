//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use std::sync::Arc;

use cardwire_config::config::CardwireConfig;
use log::{info, warn};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use zbus::{Connection, Result, proxy};

use crate::interface::ModeInterface;

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
    config: Arc<RwLock<CardwireConfig>>,
    mode_interface: ModeInterface,
) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let upower_proxy = UPowerProxy::new(&connection).await?;
    let mut battery_stream = upower_proxy.receive_on_battery_changed().await;
    // only when setting is enabled
    while let Some(msg) = battery_stream.next().await {
        let config_lock = config.read().await;
        if !config_lock.global_settings.battery_switch.enabled {
            continue;
        }
        let ac_mode = config_lock.global_settings.battery_switch.ac_mode;
        let bat_mode = config_lock.global_settings.battery_switch.bat_mode;
        drop(config_lock);
        if let Ok(state) = msg.get().await {
            info!("battery event detected: {:?}", state);
            // if state => gone to battery
            let requested = if state { bat_mode } else { ac_mode };

            // ignore dbus api error, it might happen on system with multiple gpus trying to switch
            // to hybrid, the daemon will just refuse
            if let Err(e) = mode_interface.internal_set_mode(requested, true).await {
                warn!("failed to switch mode on battery event: {e}");
            }
        }
    }

    Ok(())
}
