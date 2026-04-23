use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub target_id: String,
    pub name: String,
    pub root_path: PathBuf,
    #[serde(default = "default_target_status")]
    pub status: String,
}

impl Target {
    pub fn new(name: impl Into<String>, root_path: PathBuf) -> Self {
        let name = name.into();
        Self {
            target_id: name.clone(),
            name,
            root_path,
            status: TARGET_STATUS_ACTIVE.to_string(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == TARGET_STATUS_ACTIVE
    }

    pub fn archive(&mut self) {
        self.status = TARGET_STATUS_ARCHIVED.to_string();
    }
}

pub const TARGET_STATUS_ACTIVE: &str = "active";
pub const TARGET_STATUS_ARCHIVED: &str = "archived";

fn default_target_status() -> String {
    TARGET_STATUS_ACTIVE.to_string()
}
