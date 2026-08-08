use std::{collections::HashMap, sync::Arc};

use crate::{STATE_PATH, analyzer::AppMetadata};
use log::error;
use rusqlite::{Connection, Result};
use tokio::sync::{RwLock, mpsc, oneshot};
use zbus::zvariant;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuPolicy {
    Blocked = 0,
    Allowed = 1,
}
impl GpuPolicy {
    pub fn from_i32(val: i32) -> Self {
        match val {
            0 => GpuPolicy::Blocked,
            1 => GpuPolicy::Allowed,
            _ => GpuPolicy::Blocked,
        }
    }

    pub fn try_from_i32(val: i32) -> Option<Self> {
        match val {
            0 => Some(GpuPolicy::Blocked),
            1 => Some(GpuPolicy::Allowed),
            _ => None,
        }
    }
}

fn open_db() -> Result<Connection> {
    let db_path = format!("{}/cardwire.db", STATE_PATH);
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

#[derive(Debug, Clone, zvariant::Type, serde::Serialize, serde::Deserialize)]
pub struct DbusAppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
    pub gpu_policy: u32,
}

impl DbusAppMetadata {
    pub fn from_app_metadata(meta: &AppMetadata, gpu_policy: u32) -> Self {
        Self {
            display_name: meta.display_name.clone(),
            desktop_file_id: meta.desktop_file_id.clone(),
            icon_name: meta.icon_name.clone(),
            gpu_policy,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardwireDatabase {
    pub cache: Arc<RwLock<HashMap<String, GpuPolicy>>>,
    pub tx: mpsc::Sender<(String, AppMetadata, oneshot::Sender<bool>)>,
}
impl CardwireDatabase {
    pub fn build() -> Result<Self> {
        let conn = open_db()?;
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

        let (tx, mut rx) = mpsc::channel::<(String, AppMetadata, oneshot::Sender<bool>)>(100);

        tokio::task::spawn_blocking(move || {
            let conn = conn;
            while let Some((binary_name, meta, reply)) = rx.blocking_recv() {
                let res = conn.execute(
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
                match res {
                    Ok(_) => {
                        let _ = reply.send(true);
                    }
                    Err(err) => {
                        error!("failed to write {} to cardwire.db: {}", binary_name, err);
                        let _ = reply.send(false);
                    }
                }
            }
        });

        Ok(Self { cache, tx })
    }

    pub fn read_db(&self) -> Result<HashMap<String, DbusAppMetadata>> {
        let conn = open_db()?;

        let mut apps: HashMap<String, DbusAppMetadata> = HashMap::new();

        {
            let mut stmt = conn.prepare("SELECT binary_name, display_name, desktop_file_id, icon_name, policy FROM app_policies")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(0)?;
                let meta = DbusAppMetadata {
                    display_name: row.get(1)?,
                    desktop_file_id: row.get(2)?,
                    icon_name: row.get(3)?,
                    gpu_policy: row.get(4)?,
                };
                Ok((name, meta))
            })?;
            for row in rows.flatten() {
                apps.insert(row.0, row.1);
            }
        }

        Ok(apps)
    }
    pub fn update_policy(&self, binary_name: &str, gpu_policy: i32) -> Result<()> {
        let conn = open_db()?;

        let affected = conn.execute(
            "UPDATE app_policies SET policy = ?1 WHERE binary_name = ?2",
            rusqlite::params![gpu_policy, binary_name],
        )?;
        if affected == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_policy_from_i32_defaults_to_blocked() {
        assert_eq!(GpuPolicy::from_i32(-1), GpuPolicy::Blocked);
        assert_eq!(GpuPolicy::from_i32(42), GpuPolicy::Blocked);
        assert_eq!(GpuPolicy::try_from_i32(-1), None);
        assert_eq!(GpuPolicy::try_from_i32(42), None);
        assert_eq!(GpuPolicy::try_from_i32(1), Some(GpuPolicy::Allowed));
    }
}
