//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use log::info;
use tokio_stream::StreamExt;
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    #[zbus(signal)]
    fn on_battery(&self, state: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.github.opengamingcollective.cardwire",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
trait Cardwire {}

pub async fn watch_battery_status() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let upower_proxy = UPowerProxy::new(&connection).await?;

    let cardwire = CardwireProxy::new(&connection).await?;
    info!("Started listening to on_battery signal");
    let mut on_battery_stream = upower_proxy.receive_on_battery().await?;

    while let Some(msg) = on_battery_stream.next().await {
        println!("{:?}", msg);
    }

    Ok(())
}
