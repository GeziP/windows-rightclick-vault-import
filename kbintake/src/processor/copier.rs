use std::path::{Path, PathBuf};

use anyhow::Result;

pub fn copy_to_path(source: &Path, destination: &Path) -> Result<PathBuf> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, destination)?;
    Ok(destination.to_path_buf())
}
