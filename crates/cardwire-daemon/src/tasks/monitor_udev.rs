//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use log::{error, info};
use tokio::io::{Interest, unix::AsyncFd};

use crate::interface::DebugInterface;

pub async fn monitor_pci_changes(debug_int: DebugInterface) -> zbus::Result<()> {
    let udev_monitor = udev::MonitorBuilder::new()?.match_subsystem("pci")?;
    let udev_fd = AsyncFd::new(udev_monitor.listen()?)?;
    loop {
        let mut guard = udev_fd.ready(Interest::READABLE).await?;
        if guard.ready().is_readable() {
            for event in udev_fd.get_ref().iter() {
                if let Some(action) = event.action()
                    && (action == "bind" || action == "unbind")
                {
                    info!("detected pci event, refreshing GPU interfaces");
                    match debug_int.refresh_gpu().await {
                        Ok(()) => {}
                        Err(e) => {
                            error!("failed to refresh gpu interface: {}", e);
                        }
                    }
                }
            }
        }
        guard.clear_ready();
    }
}
