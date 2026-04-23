use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

        config.save()?;
        Ok(config)
    }

    pub fn default_target(&self) -> Result<Target> {
        self.targets
            .first()
            .cloned()
            .context("no import target configured")
    }

    pub fn target_by_id(&self, target_id: &str) -> Result<Target> {
        self.targets
            .iter()
            .find(|target| target.target_id == target_id)
            .cloned()
            .with_context(|| format!("target not configured: {target_id}"))
    }

    pub fn config_path(&self) -> PathBuf {
        self.app_data_dir.join("config.toml")
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.app_data_dir)?;
        let config_path = self.config_path();
        let encoded = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, encoded)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        Ok(())
    }

    pub fn set_default_target(
        &mut self,
        name: impl Into<String>,
        root_path: PathBuf,
    ) -> Result<Target> {
        let name = name.into();
        if name.trim().is_empty() {
            bail!("target name cannot be empty");
        }

        let target = Target::new(name, root_path);
        if self.targets.is_empty() {
            self.targets.push(target.clone());
        } else {
            self.targets[0] = target.clone();
        }
        Ok(target)
    }
}

pub fn validate_target_root(root_path: &std::path::Path) -> Result<()> {
    if root_path.exists() && !root_path.is_dir() {
        bail!(
            "target path exists but is not a directory: {}",
            root_path.display()
        );
    }

    std::fs::create_dir_all(root_path)
        .with_context(|| format!("failed to create target directory {}", root_path.display()))?;

    let probe_path = root_path.join(format!(".kbintake-write-test-{}", Uuid::new_v4()));
    std::fs::write(&probe_path, b"ok")
        .with_context(|| format!("failed to write target probe {}", probe_path.display()))?;
    std::fs::remove_file(&probe_path)
        .with_context(|| format!("failed to remove target probe {}", probe_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{validate_target_root, AppConfig};

    #[test]
    fn saves_and_reloads_updated_default_target() {
        let temp = tempfile::tempdir().unwrap();
        let app_data_dir = temp.path().join("appdata");
        let target_root = temp.path().join("vaults").join("main");

        let mut config = AppConfig::load_or_init_in(app_data_dir.clone()).unwrap();
        let target = config
            .set_default_target("main", target_root.clone())
            .unwrap();
        config.save().unwrap();

        let reloaded = AppConfig::load_or_init_in(app_data_dir).unwrap();
        assert_eq!(target.name, "main");
        assert_eq!(reloaded.targets[0].name, "main");
        assert_eq!(reloaded.targets[0].root_path, target_root);
    }

    #[test]
    fn rejects_empty_target_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        let err = config
            .set_default_target(" ", temp.path().join("vault"))
            .unwrap_err();

        assert!(err.to_string().contains("target name cannot be empty"));
    }

    #[test]
    fn looks_up_target_by_id() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .set_default_target("archive", temp.path().join("archive"))
            .unwrap();

        assert_eq!(config.target_by_id("archive").unwrap().name, "archive");
        assert!(config
            .target_by_id("missing")
            .unwrap_err()
            .to_string()
            .contains("target not configured"));
    }

    #[test]
    fn validate_target_root_creates_writable_directory() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("new-vault");

        validate_target_root(&target).unwrap();

        assert!(target.is_dir());
        assert!(fs::read_dir(target).unwrap().next().is_none());
    }

    #[test]
    fn validate_target_root_rejects_file_path() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("not-a-dir");
        fs::write(&target, "file").unwrap();

        let err = validate_target_root(&target).unwrap_err();

        assert!(err.to_string().contains("not a directory"));
    }
}
