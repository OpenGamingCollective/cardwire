mod config;
mod dbus;
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
    let mut daemon = Daemon::new().await?;
    // Now apply the config
    if let Err(e) = daemon.apply_config().await {
        log::error!("Failed to apply startup configuration: {e}");
    }
    let conn_builder = connection::Builder::system()?;
    let _conn = conn_builder
        .name("com.github.luytan.cardwire")?
        .serve_at("/com/github/luytan/cardwire", daemon)?
        .build()
        .await?;
    info!("Daemon started");
    pending::<()>().await;
    Ok(())
}
