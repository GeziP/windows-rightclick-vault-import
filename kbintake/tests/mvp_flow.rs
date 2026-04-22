use std::fs;

use kbintake::agent::scheduler::drain_queue;
use kbintake::app::App;
use kbintake::cli::handle_import;
use kbintake::queue::repository::Repository;
use kbintake::queue::state_machine;
use rusqlite::{params, Connection};

fn bootstrap_temp_app(temp: &tempfile::TempDir) -> App {
    App::bootstrap_in(temp.path().join("appdata")).unwrap()
}

#[test]
fn bootstraps_default_config_in_empty_app_data_dir() {
    let temp = tempfile::tempdir().unwrap();

    let app = bootstrap_temp_app(&temp);

    assert!(app.config.app_data_dir.join("config.toml").exists());
    assert_eq!(app.config.import.max_file_size_mb, 512);
    assert_eq!(app.config.targets.len(), 1);
    assert_eq!(app.config.targets[0].name, "default");
    assert_eq!(
        app.config.targets[0].root_path,
        app.config.app_data_dir.join("vault")
    );
}

#[test]
fn bootstrap_initializes_database_schema_idempotently() {
    let temp = tempfile::tempdir().unwrap();

    let app = bootstrap_temp_app(&temp);
    let conn = app.open_conn().unwrap();
    conn.execute(
        "INSERT INTO batches (batch_id, source, target_id, status, source_count, created_at, updated_at)
         VALUES ('batch-1', 'test', 'default', 'queued', 0, '2026-04-22T00:00:00Z', '2026-04-22T00:00:00Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let app = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
    let conn = app.open_conn().unwrap();

    for table in ["batches", "items", "manifest_records", "events"] {
        assert_eq!(sqlite_object_count(&conn, "table", table), 1);
    }
    assert_eq!(
        sqlite_object_count(&conn, "index", "idx_manifest_target_hash"),
        1
    );

    let batch_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM batches WHERE batch_id = 'batch-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(batch_count, 1);
}

#[test]
fn import_enqueue_creates_batch_and_items() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let root_file = temp.path().join("note.md");
    let nested_dir = temp.path().join("nested");
    let nested_file = nested_dir.join("child.txt");
    fs::create_dir(&nested_dir).unwrap();
    fs::write(&root_file, "root").unwrap();
    fs::write(&nested_file, "child").unwrap();

    handle_import(&app, vec![root_file, nested_dir]).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batches = repo.list_batches(10).unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].source, "cli");
    assert_eq!(batches[0].status, state_machine::STATUS_QUEUED);
    assert_eq!(batches[0].source_count, 2);

    let items = repo.list_items_by_batch(&batches[0].batch_id).unwrap();
    let mut names = items
        .iter()
        .map(|item| {
            assert_eq!(item.status, state_machine::STATUS_QUEUED);
            assert_eq!(item.target_id, "default");
            item.source_name.clone()
        })
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["child.txt", "note.md"]);
}

#[test]
fn agent_processes_queued_import_successfully() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let source = temp.path().join("note.md");
    fs::write(&source, "hello").unwrap();

    handle_import(&app, vec![source]).unwrap();
    drain_queue(&app).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();

    assert_eq!(batch.status, state_machine::STATUS_SUCCESS);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
    assert!(items[0].sha256.is_some());
    assert!(items[0].stored_path.is_some());
    assert!(app.config.targets[0].root_path.join("note.md").exists());
    assert_eq!(manifest_count_for_item(&conn, &items[0].item_id), 1);
}

#[test]
fn agent_marks_duplicate_without_second_copy() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let first = temp.path().join("first.md");
    let second = temp.path().join("second.md");
    fs::write(&first, "same").unwrap();
    fs::write(&second, "same").unwrap();

    handle_import(&app, vec![first, second]).unwrap();
    drain_queue(&app).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
    let copied_count = fs::read_dir(&app.config.targets[0].root_path)
        .unwrap()
        .count();

    assert_eq!(batch.status, state_machine::STATUS_SUCCESS);
    assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
    assert_eq!(items[1].status, state_machine::STATUS_DUPLICATE);
    assert!(items[1].duplicate_of.is_some());
    assert_eq!(copied_count, 1);
}

fn sqlite_object_count(conn: &Connection, kind: &str, name: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
        params![kind, name],
        |row| row.get(0),
    )
    .unwrap()
}

fn manifest_count_for_item(conn: &Connection, item_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM manifest_records WHERE item_id = ?1",
        params![item_id],
        |row| row.get(0),
    )
    .unwrap()
}
