//! entry point of cardwired
mod dbus;
mod file;
mod gpu_dbus;
mod listeners;
mod models;

use crate::models::Daemon;
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
    let daemon = Daemon::new().await?;

    let conn_builder = connection::Builder::system()?;
    let conn = conn_builder
        .name("com.github.opengamingcollective.cardwire")?
        .serve_at("/com/github/opengamingcollective/cardwire", daemon.clone())?
        .build()
        .await?;

    let object_server = conn.object_server();

    let gpu_list = daemon.gpu_list.read().await;

    for (id, gpu) in gpu_list.iter() {
        let path = format!("/com/github/opengamingcollective/cardwire/gpu/{}", id);
        object_server.at(path, gpu.clone()).await?;
    }
    drop(gpu_list);
    info!("Daemon started");
    pending::<()>().await;
    Ok(())
}
