use std::path::{Path, PathBuf};

use crate::processor::copier;
use anyhow::Result;

pub struct LocalFolderAdapter {
    root_path: PathBuf,
}

impl LocalFolderAdapter {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    pub fn store_copy(&self, source: &Path, source_name: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.root_path)?;
        let destination = self.available_destination(source_name);
        copier::copy_to_path(source, &destination)
    }

    pub fn preview_destination(&self, source_name: &str) -> PathBuf {
        self.available_destination(source_name)
    }

    fn available_destination(&self, source_name: &str) -> PathBuf {
        let candidate = self.root_path.join(source_name);
        if !candidate.exists() {
            return candidate;
        }

        let path = Path::new(source_name);
        let stem = path
            .file_stem()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let ext = path
            .extension()
            .map(|value| value.to_string_lossy().to_string());

        for suffix in 1.. {
            let file_name = match &ext {
                Some(ext) => format!("{stem}-{suffix}.{ext}"),
                None => format!("{stem}-{suffix}"),
            };
            let candidate = self.root_path.join(file_name);
            if !candidate.exists() {
                return candidate;
            }
        }

        unreachable!("unbounded suffix search should always return")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::LocalFolderAdapter;

    #[test]
    fn stores_first_copy_at_source_name() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("vault");
        fs::write(&source, "hello").unwrap();

        let adapter = LocalFolderAdapter::new(&target);
        let stored = adapter.store_copy(&source, "note.txt").unwrap();

        assert_eq!(stored, target.join("note.txt"));
        assert_eq!(fs::read_to_string(stored).unwrap(), "hello");
    }

    #[test]
    fn resolves_name_conflicts_deterministically() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("vault");
        fs::create_dir_all(&target).unwrap();
        fs::write(&source, "new").unwrap();
        fs::write(target.join("note.txt"), "existing").unwrap();
        fs::write(target.join("note-1.txt"), "existing again").unwrap();

        let adapter = LocalFolderAdapter::new(&target);
        let stored = adapter.store_copy(&source, "note.txt").unwrap();

        assert_eq!(stored, target.join("note-2.txt"));
        assert_eq!(
            fs::read_to_string(target.join("note.txt")).unwrap(),
            "existing"
        );
        assert_eq!(fs::read_to_string(stored).unwrap(), "new");
    }

    #[test]
    fn previews_destination_without_copying() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("vault");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("note.txt"), "existing").unwrap();

        let adapter = LocalFolderAdapter::new(&target);

        assert_eq!(
            adapter.preview_destination("note.txt"),
            target.join("note-1.txt")
        );
        assert!(!target.join("note-1.txt").exists());
    }
}
