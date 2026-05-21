//! entry point of cardwired
mod file;
mod interface;
mod listeners;
mod models;

use crate::models::DaemonManager;
use anyhow::Result;
use log::info;
use std::future::pending;
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

    let conn_builder = connection::Builder::system()?;
    let conn = conn_builder
        .name("com.github.opengamingcollective.cardwire")?
        .serve_at("/com/github/opengamingcollective/cardwire", daemon.clone())?
        .build()
        .await?;

    let object_server = conn.object_server();
    let gpu_interfaces = daemon.gpu_interfaces.read().await;
    let path = "/com/github/opengamingcollective/cardwire/Mode";
    object_server.at(path, daemon.mode_interface).await?;
    for (id, gpu_interface) in gpu_interfaces.iter() {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        object_server.at(path, gpu_interface.clone()).await?;
    }
    // drop gpu list to prevent deadlock
    drop(gpu_interfaces);
    info!("Daemon started");
    pending::<()>().await;
    Ok(())
}
