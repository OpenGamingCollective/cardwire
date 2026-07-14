//! watch fdo apps folder and refresh the fdo_list in case of new apps

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use anyhow::Result;

pub async fn watch_xdg_folders(xdg_list: Arc<RwLock<HashMap<String, bool>>>) -> Result<()> {
    Ok(())
}
