use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::adapter::local_folder::LocalFolderAdapter;
use crate::app::App;
use crate::processor::{deduper, hasher, scanner, validator};
use crate::queue::repository::Repository;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DryRunRow {
    pub source: String,
    pub destination: Option<String>,
    pub action: DryRunAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DryRunAction {
    Copy,
    SkipDuplicate,
    SkipSizeLimit,
    SkipSymlink,
}

impl DryRunAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::SkipDuplicate => "skip-duplicate",
            Self::SkipSizeLimit => "skip-size-limit",
            Self::SkipSymlink => "skip-symlink",
        }
    }
}

pub fn preview_import(
    app: &App,
    target_id: Option<String>,
    paths: Vec<PathBuf>,
) -> Result<Vec<DryRunRow>> {
    if paths.is_empty() {
        anyhow::bail!("no input paths provided");
    }

    let explicit_target = match target_id {
        Some(target_id) => Some(app.config.target_by_id(&target_id)?),
        None => None,
    };
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let mut rows = Vec::new();

    for path in paths {
        if is_symlink(&path)? {
            rows.push(row(&path, None, DryRunAction::SkipSymlink));
            continue;
        }

        let files = scanner::expand_input_path(&path)
            .with_context(|| format!("failed to scan path {}", path.display()))?;
        for file in files {
            if is_symlink(&file)? {
                rows.push(row(&file, None, DryRunAction::SkipSymlink));
                continue;
            }

            let size = match validator::validate_file(&file, app.config.import.max_file_size_mb) {
                Ok(size) => size,
                Err(err) if err.to_string().contains("exceeds max size") => {
                    rows.push(row(&file, None, DryRunAction::SkipSizeLimit));
                    continue;
                }
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("failed to validate {}", file.display()))
                }
            };

            let hash = hasher::sha256_file(&file)
                .with_context(|| format!("failed to hash {}", file.display()))?;
            let target = match &explicit_target {
                Some(target) => target.clone(),
                None => app.config.target_for_path(&file)?,
            };
            if deduper::find_duplicate_record(&repo, &target.target_id, &hash)?.is_some() {
                rows.push(row(&file, None, DryRunAction::SkipDuplicate));
                continue;
            }

            let adapter = LocalFolderAdapter::new(&target.root_path);
            let source_name = file
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            let destination = adapter.preview_destination(&source_name);
            rows.push(row(&file, Some(destination), DryRunAction::Copy));
            let _ = size;
        }
    }

    if rows.is_empty() {
        anyhow::bail!("no importable files found");
    }

    Ok(rows)
}

pub fn print_table(rows: &[DryRunRow]) {
    println!("Source Path\tDestination\tAction");
    for row in rows {
        println!(
            "{}\t{}\t{}",
            row.source,
            row.destination.as_deref().unwrap_or("-"),
            row.action.as_str()
        );
    }
}

fn row(path: &Path, destination: Option<PathBuf>, action: DryRunAction) -> DryRunRow {
    DryRunRow {
        source: path.display().to_string(),
        destination: destination.map(|path| path.display().to_string()),
        action,
    }
}

fn is_symlink(path: &Path) -> Result<bool> {
    Ok(std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .file_type()
        .is_symlink())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;

    use super::{preview_import, DryRunAction};
    use crate::app::App;
    use crate::config::{AppConfig, ImportConfig};
    use crate::db;
    use crate::domain::{ManifestRecord, Target};
    use crate::queue::repository::Repository;

    fn test_app(temp: &tempfile::TempDir) -> App {
        let app_data_dir = temp.path().join("appdata");
        let db_path = app_data_dir.join("data").join("kbintake.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        db::init_schema(&conn).unwrap();
        drop(conn);

        App {
            config: AppConfig {
                app_data_dir,
                targets: vec![Target::new("default", temp.path().join("vault"))],
                import: ImportConfig {
                    max_file_size_mb: 512,
                    inject_frontmatter: true,
                },
                routing: Vec::new(),
            },
            db_path,
        }
    }

    #[test]
    fn dry_run_copy_preview_does_not_create_file_or_batch() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();

        let rows = preview_import(&app, None, vec![source]).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        assert_eq!(rows[0].action, DryRunAction::Copy);
        assert!(rows[0].destination.as_ref().unwrap().ends_with("note.md"));
        assert!(repo.list_batches(10).unwrap().is_empty());
        assert!(!app.config.targets[0].root_path.join("note.md").exists());
    }

    #[test]
    fn dry_run_marks_duplicate_without_writing_batch() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let source = temp.path().join("note.md");
        fs::write(&source, "same").unwrap();
        let hash = crate::processor::hasher::sha256_file(&source).unwrap();
        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let record = ManifestRecord::new(
            "item".to_string(),
            "default".to_string(),
            "original.md".to_string(),
            "vault/original.md".to_string(),
            "original.md".to_string(),
            Some("md".to_string()),
            Some(4),
            hash,
        );
        repo.insert_manifest(&record).unwrap();

        let rows = preview_import(&app, None, vec![source]).unwrap();

        assert_eq!(rows[0].action, DryRunAction::SkipDuplicate);
        assert!(rows[0].destination.is_none());
        assert!(repo.list_batches(10).unwrap().is_empty());
    }

    #[test]
    fn dry_run_marks_size_limit() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config.import.max_file_size_mb = 0;
        let source = temp.path().join("large.md");
        fs::write(&source, "too large").unwrap();

        let rows = preview_import(&app, None, vec![source]).unwrap();

        assert_eq!(rows[0].action, DryRunAction::SkipSizeLimit);
        assert!(rows[0].destination.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn dry_run_marks_symlink_input() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let source = temp.path().join("source.md");
        let link = temp.path().join("link.md");
        fs::write(&source, "hello").unwrap();
        if std::os::windows::fs::symlink_file(&source, &link).is_err() {
            return;
        }

        let rows = preview_import(&app, None, vec![link]).unwrap();

        assert_eq!(rows[0].action, DryRunAction::SkipSymlink);
        assert!(rows[0].destination.is_none());
    }
}
