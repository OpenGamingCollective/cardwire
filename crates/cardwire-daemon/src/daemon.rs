//! entry point of cardwired
mod analyzer;
mod core;
mod file;
mod interface;
mod manager;
mod tasks;
pub mod types;

use crate::{manager::DaemonManager, tasks::watch_power_state};
use anyhow::Result;
use env_logger::Env;
use log::info;
use std::{future::pending, sync::Arc};
use tokio::task;
use zbus::connection;

/// Cardwire configuration directory.
pub const CONFIG_PATH: &str = "/etc/cardwire";
/// Cardwire state directory.
pub const STATE_PATH: &str = "/var/lib/cardwire";

#[tokio::main]
async fn main() -> Result<()> {
    // log
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    // Build the DaemonManager, it mostly consists of reading config files and setting up Arc and
    // RwLocks
    let mut daemon = DaemonManager::new().await?;
    // Before we publish the API
    daemon.pre_daemon_tasks().await?;

    // Now connect to the system dbus
    let conn_builder = connection::Builder::system()?;
    let conn = conn_builder
        .name("org.opengamingcollective.cardwire")?
        .serve_at("/org/opengamingcollective/cardwire", daemon.clone())?
        .serve_at(
            "/org/opengamingcollective/cardwire",
            zbus::fdo::ObjectManager,
        )?
        .replace_existing_names(false)
        .allow_name_replacements(false)
        .build()
        .await?;

    let _conn = match async {
        let conn_builder = connection::Builder::system()?;
        conn_builder
            .name("net.hadess.SwitcherooControl")?
            .serve_at(
                "/net/hadess/SwitcherooControl",
                daemon.switcheroo_interface.clone(),
            )?
            .replace_existing_names(true)
            .build()
            .await
    }
    .await
    {
        Ok(connection) => {
            info!("Switcheroo DBus shim started successfully");
            Some(connection)
        }
        Err(e) => {
            // If the DBus policy blocks it, or Switcheroo is already running,
            log::warn!("Failed to start Switcheroo D-Bus shim: {}", e);
            None
        }
    };

    // Give the shim its signal emitter so it can notify clients (e.g. gnome) when the GPU list
    // changes
    if let Some(switcheroo_conn) = _conn.as_ref() {
        match switcheroo_conn
            .object_server()
            .interface::<_, crate::interface::SwitcherooInterface>("/net/hadess/SwitcherooControl")
            .await
        {
            Ok(switcheroo_ref) => {
                daemon
                    .switcheroo_interface
                    .signal_emitter
                    .get_or_init(|| switcheroo_ref.signal_emitter().to_owned());
            }
            Err(e) => {
                log::warn!(
                    "Failed to get the Switcheroo shim interface ({e}); GPU change notifications will not be emitted"
                );
            }
        }
    }

    let object_server: &zbus::ObjectServer = conn.object_server();
    spawn_dbus_api(object_server, &mut daemon).await?;
    // Spawn background tasks
    // Give the Mode interface its signal emitter so automatic transitions (which bypass the
    // D-Bus property setter) can notify clients.
    if let Ok(mode_ref) = object_server
        .interface::<_, crate::interface::ModeInterface>("/org/opengamingcollective/cardwire")
        .await
    {
        daemon
            .mode_interface
            .signal_emitter
            .get_or_init(|| mode_ref.signal_emitter().to_owned());
    }
    task::spawn(daemon.battery_switch_future());
    task::spawn(daemon.monitor_udev_future());
    task::spawn(daemon.monitor_display_future());
    task::spawn(daemon.run_analyzer());
    info!("Daemon started succesfully");
    pending::<()>().await;
    Ok(())
}

async fn spawn_dbus_api(
    object_server: &zbus::ObjectServer,
    daemon: &mut DaemonManager,
) -> anyhow::Result<()> {
    let path = "/org/opengamingcollective/cardwire";

    let gpu_interfaces = daemon.inner.gpu_list.read().await;
    // cardwire.Mode
    object_server
        .at(path, daemon.mode_interface.clone())
        .await?;
    // cardwire.Config
    object_server
        .at(path, daemon.config_interface.clone())
        .await?;
    // cardwire.Gpu
    let mut power_tasks = daemon.inner.power_tasks.write().await;
    for (id, gpu_interface) in gpu_interfaces.iter() {
        let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
        object_server
            .at(path.clone(), gpu_interface.as_ref().clone())
            .await?;
        // spawn power state watcher only for available GPUs
        if gpu_interface.device.is_available() {
            let handle = task::spawn(watch_power_state(
                Arc::clone(gpu_interface),
                object_server.interface(path).await?,
            ));
            power_tasks.insert(*id, handle);
        }
    }
    // Cardwire logger
    object_server
        .at(path, daemon.logger_interface.clone())
        .await?;
    if let Ok(logger_ref) = object_server
        .interface::<_, crate::interface::LoggerInterface>(path)
        .await
    {
        daemon
            .logger_interface
            .signal_emitter
            .get_or_init(|| logger_ref.signal_emitter().to_owned());
    }

    // Cardwire Smart Policy
    object_server
        .at(path, daemon.smart_policy_interface.clone())
        .await?;

    drop(power_tasks);
    // drop gpu list to prevent deadlock
    drop(gpu_interfaces);
    // give the server to the debug interface
    daemon.debug_interface.object_server = Some(object_server.to_owned());
    // cardwire.Debug
    object_server
        .at(path, daemon.debug_interface.clone())
        .await?;
    Ok(())
}
