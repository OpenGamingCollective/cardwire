//! SQLite helper for cardwired

use std::{collections::BTreeMap, path::Path};

use rusqlite::Connection;
const STATE_PATH: &str = "/var/lib/cardwire";

pub struct CardwireDatabase {
    apps: BTreeMap<String, App>,
}
#[derive(Clone)]
pub struct App {
    pub comm_name: String,
    pub blocked: bool,
}
impl CardwireDatabase {
    pub fn build() -> anyhow::Result<CardwireDatabase> {
        let db_path = format!("{STATE_PATH}/app_list.db3");
        let db_path = Path::new(&db_path);
        let conn = Connection::open(db_path)?;

        conn.execute(
            "CREATE TABLE if not exists apps (
                id  INTEGER PRIMARY KEY,
                name    TEXT NOT NULL,
                comm_name TEXT NOT NULL,
                blocked INT CHECK (blocked IN (0, 1))
            ) STRICT",
            (),
        )?;

        let mut apps: BTreeMap<String, App> = BTreeMap::new();
        // example for discord
        let discord = App {
            comm_name: "nvtop".to_string(),
            blocked: false,
        };
        apps.insert("Discord".to_string(), discord);

        Ok(CardwireDatabase { apps })
    }
    pub async fn insert_app(
        &mut self,
        name: String,
        comm_name: String,
        blocked: bool,
    ) -> anyhow::Result<()> {
        let app = App { comm_name, blocked };
        // only insert if key doesnt exist
        if !self.apps.contains_key(&name) {
            // insert into map first
            self.apps.insert(name.clone(), app.clone());

            // then into db
            let db_path = format!("{STATE_PATH}/app_list.db3");
            let db_path = Path::new(&db_path);
            let conn = Connection::open(db_path)?;
            conn.execute(
                "INSERT INTO apps (name, comm_name, blocked) VALUES (?1, ?2, ?3)",
                (&name, &app.comm_name, &app.blocked),
            )?;
        }
        Ok(())
    }
}
