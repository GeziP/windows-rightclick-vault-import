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
        Self::bootstrap_with_config(config)
    }

    pub fn bootstrap_in(app_data_dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let config = AppConfig::load_or_init_in(app_data_dir.into())?;
        Self::bootstrap_with_config(config)
    }

    fn bootstrap_with_config(config: AppConfig) -> Result<Self> {
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
