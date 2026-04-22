use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub target_id: String,
    pub name: String,
    pub root_path: PathBuf,
}

impl Target {
    pub fn new(name: impl Into<String>, root_path: PathBuf) -> Self {
        let name = name.into();
        Self {
            target_id: name.clone(),
            name,
            root_path,
        }
    }
}
