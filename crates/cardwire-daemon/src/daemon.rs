//! entry point of cardwired
mod analyzer;
mod core;
mod file;
mod interface;
mod models;
mod tasks;

use crate::{models::DaemonManager, tasks::watch_power_state};
use anyhow::Result;
use env_logger::Env;
use log::info;
use std::future::pending;
use tokio::task;
use zbus::connection;
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

    let object_server: &zbus::ObjectServer = conn.object_server();
    spawn_dbus_api(object_server, &mut daemon).await?;
    // Spawn background tasks
    // Automatic transitions happen outside a D-Bus call, so the monitor needs this reference to
    // emit Mode property changes to clients.
    let mode_interface = object_server
        .interface::<_, crate::interface::ModeInterface>("/org/opengamingcollective/cardwire")
        .await?;
    task::spawn(daemon.battery_switch_future());
    task::spawn(daemon.monitor_udev_future());
    task::spawn(daemon.monitor_display_future(mode_interface));
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

    let gpu_interfaces = daemon.gpu_interfaces.read().await;
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
            .at(path.clone(), gpu_interface.clone())
            .await?;
        // spawn power state watcher only for available GPUs
        if gpu_interface.device.is_available() {
            let handle = task::spawn(watch_power_state(
                gpu_interface.clone(),
                object_server.interface(path).await?,
            ));
            power_tasks.insert(*id, handle);
        }
    }
    // Cardwire logger
    object_server
        .at(path, daemon.logger_interface.clone())
        .await?;
    let logger_ref = object_server
        .interface::<_, crate::interface::LoggerInterface>(path)
        .await?;
    daemon.logger_signal = Some(logger_ref.signal_emitter().clone());

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
