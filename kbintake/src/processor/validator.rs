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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::validate_file;

    #[test]
    fn accepts_file_within_size_limit() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("note.md");
        fs::write(&file, "hello").unwrap();

        assert_eq!(validate_file(&file, 1).unwrap(), 5);
    }

    #[test]
    fn rejects_missing_file() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing.md");

        let err = validate_file(&missing, 1).unwrap_err();

        assert!(err.to_string().contains("source file does not exist"));
    }

    #[test]
    fn rejects_directory_source() {
        let temp = tempfile::tempdir().unwrap();

        let err = validate_file(temp.path(), 1).unwrap_err();

        assert!(err.to_string().contains("source path is not a file"));
    }

    #[test]
    fn rejects_file_above_size_limit() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("large.bin");
        fs::write(&file, b"x").unwrap();

        let err = validate_file(&file, 0).unwrap_err();

        assert!(err.to_string().contains("source file exceeds max size"));
    }
}
