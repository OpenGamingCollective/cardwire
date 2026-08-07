use std::{collections::HashMap, sync::Arc};

use crate::{analyzer::AppMetadata, file::state::STATE_PATH};
use log::error;
use rusqlite::{Connection, Result};
use tokio::sync::{RwLock, mpsc};

#[derive(Debug, Clone, PartialEq)]
pub enum GpuPolicy {
    Allowed = 0,
    Blocked = 1,
    Forced = 2,
}
impl GpuPolicy {
    pub fn from_i32(val: i32) -> Self {
        match val {
            0 => GpuPolicy::Allowed,
            1 => GpuPolicy::Forced,
            2 => GpuPolicy::Blocked,
            _ => GpuPolicy::Allowed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardwireDatabase {
    pub cache: Arc<RwLock<HashMap<String, GpuPolicy>>>,
    pub tx: mpsc::Sender<(String, AppMetadata)>,
}
impl CardwireDatabase {
    pub fn build() -> Result<Self> {
        let db_path = format!("{}/cardwire.db", STATE_PATH);

        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS app_policies (
                binary_name TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                desktop_file_id TEXT,
                icon_name TEXT,
                policy INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        let mut cache_map = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT binary_name, policy FROM app_policies")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(0)?;
                let policy: i32 = row.get(1)?;
                Ok((name, GpuPolicy::from_i32(policy)))
            })?;
            for row in rows.flatten() {
                cache_map.insert(row.0, row.1);
            }
        }

        let cache = Arc::new(RwLock::new(cache_map));

        let (tx, mut rx) = mpsc::channel::<(String, AppMetadata)>(100);

        tokio::task::spawn_blocking(move || {
            let mut _conn = conn;
            while let Some((binary_name, meta)) = rx.blocking_recv() {
                let res = _conn.execute(
                    "INSERT INTO app_policies (binary_name, display_name, desktop_file_id, icon_name, policy)
                     VALUES (?1, ?2, ?3, ?4, 0)
                     ON CONFLICT(binary_name) DO NOTHING",
                    rusqlite::params![
                        binary_name,
                        meta.display_name,
                        meta.desktop_file_id,
                        meta.icon_name
                    ],
                );
                if let Err(err) = res {
                    error!("failed to write {} to cardwire.db: {}", binary_name, err);
                }
            }
        });

        Ok(Self { cache, tx })
    }
}
