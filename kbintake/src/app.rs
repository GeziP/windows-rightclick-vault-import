use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::db;

pub struct App {
    pub config: AppConfig,
    pub db_path: std::path::PathBuf,
}

impl App {
    pub fn bootstrap() -> Result<Self> {
        let config = AppConfig::load_or_init().context("failed to load or initialize config")?;
        Self::bootstrap_with_config(config)
    }

    pub fn bootstrap_in(app_data_dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let app_data_dir = app_data_dir.into();
        let config = AppConfig::load_or_init_in(app_data_dir.clone()).with_context(|| {
            format!(
                "failed to load or initialize config in {}",
                app_data_dir.display()
            )
        })?;
        Self::bootstrap_with_config(config)
    }

    fn bootstrap_with_config(config: AppConfig) -> Result<Self> {
        let db_path = config.app_data_dir.join("data").join("kbintake.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory {}", parent.display())
            })?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open database {}", db_path.display()))?;
        db::init_schema(&conn).context("failed to initialize database schema")?;
        drop(conn);

        Ok(Self { config, db_path })
    }

    pub fn open_conn(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .with_context(|| format!("failed to open database {}", self.db_path.display()))
    }
}
