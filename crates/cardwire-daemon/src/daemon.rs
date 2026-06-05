//! entry point of cardwired
mod core;
mod file;
mod interface;
mod models;
mod profiler;
mod tasks;

use crate::{models::DaemonManager, tasks::watch_power_state};
use anyhow::Result;
use env_logger::Env;
use log::info;
use std::{future::pending, sync::Arc};
use tokio::task;
use zbus::connection;
#[tokio::main]
async fn main() -> Result<()> {
    // log
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();
    let daemon = DaemonManager::new().await?;
    // Before we publish the API
    daemon.pre_daemon_tasks().await?;

    // Prepare the future before moving debug
    let battery_switch = tasks::watch_battery_status(Arc::clone(
        &daemon.debug_interface.config.battery_auto_switch,
    ));
    let conn_builder = connection::Builder::system()?;
    let conn = conn_builder
        .name("com.github.opengamingcollective.cardwire")?
        .serve_at("/com/github/opengamingcollective/cardwire", daemon.clone())?
        .serve_at(
            "/com/github/opengamingcollective/cardwire",
            zbus::fdo::ObjectManager,
        )?
        .build()
        .await?;

    let object_server: &zbus::ObjectServer = conn.object_server();
    spawn_dbus_api(object_server, &daemon).await?;
    // Now spawn background tasks
    task::spawn(battery_switch);
    task::spawn(daemon.cardwire_profiler.spawn_profiler());

    info!("Daemon started");
    pending::<()>().await;
    Ok(())
}

async fn spawn_dbus_api(
    object_server: &zbus::ObjectServer,
    daemon: &DaemonManager,
) -> anyhow::Result<()> {
    let path = "/com/github/opengamingcollective/cardwire";

    let gpu_interfaces = daemon.gpu_interfaces.read().await;
    // cardwire.Mode
    object_server
        .at(path, daemon.mode_interface.clone())
        .await?;
    // cardwire.Config
    object_server
        .at(path, daemon.config_interface.clone())
        .await?;
    // cardwire.Debug
    object_server
        .at(path, daemon.debug_interface.clone())
        .await?;
    // cardwire.Gpu
    for (id, gpu_interface) in gpu_interfaces.iter() {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        object_server
            .at(path.clone(), gpu_interface.clone())
            .await?;
        // spawn power state watcher
        task::spawn(watch_power_state(
            gpu_interface.clone(),
            object_server.interface(path).await?,
        ));
    }
    // drop gpu list to prevent deadlock
    drop(gpu_interfaces);
    Ok(())
}
