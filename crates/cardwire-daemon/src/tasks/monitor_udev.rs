//! Used to listen to other dbus interface, mainly for auto battery switch and display detection

use crate::interface::DebugInterface;
use log::{error, info};
use tokio::io::unix::AsyncFd;

/// listen to pci/thunderbolt event and refres the GPU list when necessary
pub async fn monitor_pci_changes(debug_int: DebugInterface) -> zbus::Result<()> {
    let udev_pci_monitor = udev::MonitorBuilder::new()?.match_subsystem("pci")?;
    let udev_pci_fd = AsyncFd::new(udev_pci_monitor.listen()?)?;
    let udev_thunderbolt_monitor = udev::MonitorBuilder::new()?.match_subsystem("thunderbolt")?;
    let udev_thunderbolt_fd = AsyncFd::new(udev_thunderbolt_monitor.listen()?)?;
    loop {
        tokio::select! {
            guard_res = udev_pci_fd.readable() => {
                if let Ok(mut guard) = guard_res && guard.ready().is_readable() {
                    for event in udev_pci_fd.get_ref().iter() {
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
                    guard.clear_ready();
                }
            }
            guard_res = udev_thunderbolt_fd.readable() => {
                if let Ok(mut guard) = guard_res && guard.ready().is_readable() {
                    for event in udev_thunderbolt_fd.get_ref().iter() {
                        if let Some(action) = event.action()
                            && (action == "add" || action == "remove" || action == "change")
                            // try to match most actions, refreshing isnt that ressource intensive
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
                    guard.clear_ready();
                }
            }
        }
    }
}
