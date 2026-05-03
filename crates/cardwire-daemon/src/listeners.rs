//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use log::info;
use tokio_stream::StreamExt;
use zbus::{Connection, Result, proxy};

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
    interface = "com.github.opengamingcollective.cardwire",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
trait Cardwire {
    fn set_mode(&self, mode: u32) -> Result<()>;
}

pub async fn watch_battery_status() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let upower_proxy = UPowerProxy::new(&connection).await?;

    let cardwire = CardwireProxy::new(&connection).await?;
    info!("Started listening to on_battery property");
    let mut battery_stream = upower_proxy.receive_on_battery_changed().await;

    while let Some(msg) = battery_stream.next().await {
        if let Ok(state) = msg.get().await {
            info!("battery event detected: {:?}", state);
            match state {
                true => cardwire.set_mode(0).await?,
                false => cardwire.set_mode(1).await?,
            };
        }
    }

    Ok(())
}
