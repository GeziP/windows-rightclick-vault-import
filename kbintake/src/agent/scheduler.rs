use anyhow::Result;
use tracing::info;

use crate::agent::worker;
use crate::app::App;
use crate::queue::repository::Repository;
use crate::queue::state_machine;

pub fn drain_queue(app: &App) -> Result<()> {
    let mut processed = 0usize;

    while process_next_item(app)? {
        processed += 1;
    }

    info!(processed, "agent queue drain completed");
    println!("Processed items: {processed}");
    Ok(())
}

pub fn process_next_item(app: &App) -> Result<bool> {
    let next = {
        let conn = app.open_conn()?;
        let repo = Repository::new(&conn);
        repo.next_queued_item()?
    };

    let Some(item) = next else {
        return Ok(false);
    };

    {
        let conn = app.open_conn()?;
        let repo = Repository::new(&conn);
        repo.update_batch_status(&item.batch_id, state_machine::STATUS_RUNNING)?;
    }

    let batch_id = item.batch_id.clone();
    worker::process_item(app, item)?;

    {
        let conn = app.open_conn()?;
        let repo = Repository::new(&conn);
        repo.refresh_batch_status(&batch_id)?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;

    use super::{drain_queue, process_next_item};
    use crate::app::App;
    use crate::config::{AgentConfig, AppConfig, ImportConfig};
    use crate::db;
    use crate::domain::{BatchJob, ItemJob, Target};
    use crate::queue::repository::Repository;
    use crate::queue::state_machine;

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
                    language: None,
                    auto_open_obsidian: false,
                },
                agent: AgentConfig {
                    poll_interval_secs: 5,
                    watch_in_service: false,
                },
                routing: Vec::new(),
                templates: Vec::new(),
                routing_rules: Vec::new(),
                watch: Vec::new(),
            },
            db_path,
        }
    }

    fn insert_batch_with_items(app: &App, files: Vec<std::path::PathBuf>) -> String {
        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let target = app.config.default_target().unwrap();
        let batch = BatchJob::new("test", &target.target_id, files.len() as i64);
        let batch_id = batch.batch_id.clone();
        repo.insert_batch(&batch).unwrap();
        for file in files {
            repo.insert_item(&ItemJob::new(
                batch_id.clone(),
                target.target_id.clone(),
                file,
            ))
            .unwrap();
        }
        batch_id
    }

    fn insert_batch_with_target(
        app: &App,
        target_id: &str,
        files: Vec<std::path::PathBuf>,
    ) -> String {
        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = BatchJob::new("test", target_id, files.len() as i64);
        let batch_id = batch.batch_id.clone();
        repo.insert_batch(&batch).unwrap();
        for file in files {
            repo.insert_item(&ItemJob::new(batch_id.clone(), target_id.to_string(), file))
                .unwrap();
        }
        batch_id
    }

    #[test]
    fn process_next_item_returns_false_when_queue_is_empty() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);

        assert!(!process_next_item(&app).unwrap());
    }

    #[test]
    fn drain_queue_imports_valid_item_and_marks_batch_success() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();
        let batch_id = insert_batch_with_items(&app, vec![source]);

        drain_queue(&app).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.get_batch(&batch_id).unwrap();
        let items = repo.list_items_by_batch(&batch_id).unwrap();

        assert_eq!(batch.status, state_machine::STATUS_SUCCESS);
        assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
        assert!(app
            .config
            .default_target()
            .unwrap()
            .root_path
            .join("note.md")
            .exists());
    }

    #[test]
    fn drain_queue_marks_later_duplicate_without_extra_copy() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let first = temp.path().join("first.md");
        let second = temp.path().join("second.md");
        fs::write(&first, "same").unwrap();
        fs::write(&second, "same").unwrap();
        let batch_id = insert_batch_with_items(&app, vec![first, second]);

        drain_queue(&app).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.get_batch(&batch_id).unwrap();
        let items = repo.list_items_by_batch(&batch_id).unwrap();
        let copied_count = fs::read_dir(app.config.default_target().unwrap().root_path)
            .unwrap()
            .count();

        assert_eq!(batch.status, state_machine::STATUS_SUCCESS);
        assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
        assert_eq!(items[1].status, state_machine::STATUS_DUPLICATE);
        assert!(items[1].duplicate_of.is_some());
        assert_eq!(copied_count, 1);
    }

    #[test]
    fn drain_queue_marks_invalid_item_and_batch_failed() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let missing = temp.path().join("missing.md");
        let batch_id = insert_batch_with_items(&app, vec![missing]);

        drain_queue(&app).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.get_batch(&batch_id).unwrap();
        let items = repo.list_items_by_batch(&batch_id).unwrap();

        assert_eq!(batch.status, state_machine::STATUS_FAILED);
        assert_eq!(items[0].status, state_machine::STATUS_FAILED);
        assert_eq!(items[0].error_code.as_deref(), Some("E_SOURCE_INVALID"));
    }

    #[test]
    fn drain_queue_uses_item_target_not_current_default() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config.targets = vec![
            Target::new("new-default", temp.path().join("new-vault")),
            Target::new("old-default", temp.path().join("old-vault")),
        ];
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();
        let batch_id = insert_batch_with_target(&app, "old-default", vec![source]);

        drain_queue(&app).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let items = repo.list_items_by_batch(&batch_id).unwrap();

        assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
        assert!(temp.path().join("old-vault").join("note.md").exists());
        assert!(!temp.path().join("new-vault").join("note.md").exists());
    }

    #[test]
    fn drain_queue_marks_missing_target_failed() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();
        let batch_id = insert_batch_with_target(&app, "missing-target", vec![source]);

        drain_queue(&app).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.get_batch(&batch_id).unwrap();
        let items = repo.list_items_by_batch(&batch_id).unwrap();

        assert_eq!(batch.status, state_machine::STATUS_FAILED);
        assert_eq!(items[0].status, state_machine::STATUS_FAILED);
        assert_eq!(items[0].error_code.as_deref(), Some("E_TARGET_MISSING"));
    }
}
