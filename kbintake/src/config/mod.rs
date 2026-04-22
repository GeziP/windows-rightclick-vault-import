use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::Target;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_data_dir: PathBuf,
    pub targets: Vec<Target>,
    pub import: ImportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    pub max_file_size_mb: u64,
}

impl AppConfig {
    pub fn load_or_init() -> Result<Self> {
        let app_data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kbintake");
        Self::load_or_init_in(app_data_dir)
    }

    pub fn load_or_init_in(app_data_dir: PathBuf) -> Result<Self> {
        let config_path = app_data_dir.join("config.toml");

        if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            let config = toml::from_str(&raw)
                .with_context(|| format!("failed to parse {}", config_path.display()))?;
            return Ok(config);
        }

        std::fs::create_dir_all(&app_data_dir)?;
        let target_root = app_data_dir.join("vault");
        let config = Self {
            app_data_dir,
            targets: vec![Target::new("default", target_root)],
            import: ImportConfig {
                max_file_size_mb: 512,
            },
        };

        let encoded = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, encoded)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        Ok(config)
    }

    pub fn default_target(&self) -> Result<Target> {
        self.targets
            .first()
            .cloned()
            .context("no import target configured")
    }
}
