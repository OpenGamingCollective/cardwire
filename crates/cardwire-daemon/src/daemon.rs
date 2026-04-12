mod config;
mod dbus;
mod models;

use crate::models::Daemon;
use anyhow::Result;
use config::Config;
use log::info;
use std::future::pending;
use zbus::connection;
#[tokio::main]
async fn main() -> Result<()> {
    // log
    env_logger::builder()
        .format_timestamp_nanos()
        .filter_level(log::LevelFilter::Info)
        .init();
    let daemon = Daemon::new(Config::new().await)?;

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
