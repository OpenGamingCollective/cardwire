//! entry point of cardwired
mod file;
mod interface;
mod models;
mod tasks;

use crate::models::DaemonManager;
use anyhow::Result;
use log::info;
use std::{future::pending, sync::Arc};
use tokio::task;
use zbus::connection;
#[tokio::main]
async fn main() -> Result<()> {
    // log
    env_logger::Builder::from_default_env()
        .format_target(false)
        .format_timestamp(None)
        .filter_level(log::LevelFilter::Info)
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

    let object_server = conn.object_server();
    let gpu_interfaces = daemon.gpu_interfaces.read().await;
    let path = "/com/github/opengamingcollective/cardwire";
    // cardwire.Mode
    object_server.at(path, daemon.mode_interface).await?;
    // cardwire.Config
    object_server.at(path, daemon.config_interface).await?;
    // cardwire.Debug
    object_server.at(path, daemon.debug_interface).await?;
    // cardwire.Gpu
    for (id, gpu_interface) in gpu_interfaces.iter() {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        object_server.at(path, gpu_interface.clone()).await?;
    }
    // drop gpu list to prevent deadlock
    drop(gpu_interfaces);

    // Now spawn background tasks
    let _ = task::spawn(battery_switch);

    info!("Daemon started");
    pending::<()>().await;
    Ok(())
}
