use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use walkdir::WalkDir;

pub fn expand_input_path(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        bail!("input path does not exist: {}", path.display());
    }

    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_dir() {
        let mut files = Vec::new();
        for entry in WalkDir::new(path).follow_links(false) {
            let entry = entry?;
            if entry.file_type().is_file() {
                files.push(entry.path().to_path_buf());
            }
        }
        return Ok(files);
    }

    bail!(
        "input path is neither file nor directory: {}",
        path.display()
    );
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::expand_input_path;

    #[test]
    fn expands_single_file() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("note.md");
        fs::write(&file, "hello").unwrap();

        assert_eq!(expand_input_path(&file).unwrap(), vec![file]);
    }

    #[test]
    fn expands_nested_directory_files() {
        let temp = tempfile::tempdir().unwrap();
        let root_file = temp.path().join("root.md");
        let nested_dir = temp.path().join("nested");
        let nested_file = nested_dir.join("child.md");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(&root_file, "root").unwrap();
        fs::write(&nested_file, "child").unwrap();

        let mut files = expand_input_path(temp.path()).unwrap();
        files.sort();
        let mut expected = vec![root_file, nested_file];
        expected.sort();

        assert_eq!(files, expected);
    }

    #[test]
    fn rejects_missing_input_path() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing.md");

        let err = expand_input_path(&missing).unwrap_err();

        assert!(err.to_string().contains("input path does not exist"));
    }
}
