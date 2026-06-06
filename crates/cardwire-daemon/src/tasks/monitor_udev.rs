//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use log::info;
use tokio::io::{Interest, unix::AsyncFd};
use zbus::{Connection, Result, proxy};

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Debug",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
trait Cardwire {
    fn refresh_gpu(&self) -> Result<()>;
}

pub async fn monitor_pci_changes() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let udev_monitor = udev::MonitorBuilder::new()?.match_subsystem("pci")?;
    let udev_fd = AsyncFd::new(udev_monitor.listen()?)?;
    let cardwire = CardwireProxy::new(&connection).await?;
    loop {
        let mut guard = udev_fd.ready(Interest::READABLE).await?;
        if guard.ready().is_readable() {
            for event in udev_fd.get_ref().iter() {
                if let Some(action) = event.action()
                    && (action == "add" || action == "unbind")
                {
                    info!("detected pci event, refreshing GPU interfaces");
                    cardwire.refresh_gpu().await?;
                }
            }
        }
        guard.clear_ready();
    }
}
