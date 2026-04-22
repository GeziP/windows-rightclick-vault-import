use std::path::Path;

use anyhow::{bail, Result};

pub fn validate_file(path: &Path, max_file_size_mb: u64) -> Result<u64> {
    if !path.exists() {
        bail!("source file does not exist: {}", path.display());
    }
    if !path.is_file() {
        bail!("source path is not a file: {}", path.display());
    }

    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();
    let max_bytes = max_file_size_mb.saturating_mul(1024).saturating_mul(1024);
    if size > max_bytes {
        bail!("source file exceeds max size of {} MB", max_file_size_mb);
    }

    Ok(size)
}
