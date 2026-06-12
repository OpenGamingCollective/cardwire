//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use std::sync::{Arc, atomic::AtomicBool};

use log::info;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use zbus::{Connection, Result, proxy};

use crate::interface::Modes;

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    #[zbus(property)]
    fn on_battery(&self) -> Result<bool>;
}
#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Mode",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
trait Cardwire {
    #[zbus(property)]
    fn set_mode(&self, mode: u32) -> Result<()>;
}

pub async fn watch_battery_status(
    switch_setting: Arc<AtomicBool>,
    switch_mode: Arc<RwLock<Modes>>,
) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let upower_proxy = UPowerProxy::new(&connection).await?;

    let cardwire = CardwireProxy::new(&connection).await?;
    let mut battery_stream = upower_proxy.receive_on_battery_changed().await;
    let mode = switch_mode.read().await;
    info!(
        "Started listening to on_battery property with mode to set on ac: {}",
        *mode
    );
    drop(mode);
    // only when setting is enabled
    while let Some(msg) = battery_stream.next().await {
        if !switch_setting.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }
        if let Ok(state) = msg.get().await {
            info!("battery event detected: {:?}", state);
            // now get the configured mode and change
            let mode = switch_mode.read().await;
            let mode_u32 = Modes::parse_to_u32(*mode);
            // ignore dbus api error, it might happen on system with multiple gpus trying to switch
            // to hybrid, the daemon will just refuse
            let _ = match state {
                true => cardwire.set_mode(0).await,
                false => cardwire.set_mode(mode_u32).await,
            };
            // just to be sure
            drop(mode);
        }
    }

    Ok(())
}
