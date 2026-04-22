use anyhow::Result;
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::db;

pub struct App {
    pub config: AppConfig,
    pub db_path: std::path::PathBuf,
}

impl App {
    pub fn bootstrap() -> Result<Self> {
        let config = AppConfig::load_or_init()?;
        let db_path = config.app_data_dir.join("data").join("kbintake.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        db::init_schema(&conn)?;
        drop(conn);

        Ok(Self { config, db_path })
    }

    pub fn open_conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }
}
