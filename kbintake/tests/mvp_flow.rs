use std::fs;
use std::process::Command;

use kbintake::agent::scheduler::drain_queue;
use kbintake::app::App;
use kbintake::cli::{
    handle_import, handle_import_command, handle_jobs, handle_targets, JobCommands, TargetCommands,
};
use kbintake::queue::repository::Repository;
use kbintake::queue::state_machine;
use rusqlite::{params, Connection};

fn kbintake_command(app_data_dir: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kbintake"));
    command.env("KBINTAKE_APP_DATA_DIR", app_data_dir);
    command
}

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
    for index in [
        "idx_manifest_target_hash",
        "idx_batches_created_at",
        "idx_items_batch",
        "idx_items_status_created_at",
        "idx_items_target_hash",
    ] {
        assert_eq!(sqlite_object_count(&conn, "index", index), 1);
    }
    kbintake::db::validate_schema(&conn).unwrap();

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

    handle_import(&app, None, vec![root_file, nested_dir]).unwrap();

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

    handle_import(&app, None, vec![source]).unwrap();
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
    assert_eq!(
        event_types_for_entity(&conn, "batch", &batch.batch_id),
        vec!["batch.queued"]
    );
    assert_eq!(
        event_types_for_entity(&conn, "item", &items[0].item_id),
        vec!["item.queued", "item.success"]
    );
}

#[test]
fn agent_marks_duplicate_without_second_copy() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let first = temp.path().join("first.md");
    let second = temp.path().join("second.md");
    fs::write(&first, "same").unwrap();
    fs::write(&second, "same").unwrap();

    handle_import(&app, None, vec![first, second]).unwrap();
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
    assert_eq!(
        event_types_for_entity(&conn, "item", &items[1].item_id),
        vec!["item.queued", "item.duplicate"]
    );
}

#[test]
fn explicit_import_target_processes_into_selected_vault() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    handle_targets(
        &app,
        TargetCommands::Add {
            name: "archive".to_string(),
            path: temp.path().join("archive-vault"),
        },
    )
    .unwrap();
    let app = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
    let source = temp.path().join("archive-note.md");
    fs::write(&source, "hello archive").unwrap();

    handle_import(&app, Some("archive".to_string()), vec![source]).unwrap();
    drain_queue(&app).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();

    assert_eq!(batch.target_id, "archive");
    assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
    assert!(temp
        .path()
        .join("archive-vault")
        .join("archive-note.md")
        .exists());
    assert!(!app
        .config
        .targets
        .iter()
        .find(|target| target.target_id == "default")
        .unwrap()
        .root_path
        .join("archive-note.md")
        .exists());
}

#[test]
fn import_process_drains_new_work_end_to_end() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let source = temp.path().join("process-note.md");
    fs::write(&source, "process me").unwrap();

    handle_import_command(&app, None, true, false, false, vec![source]).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();

    assert_eq!(batch.status, state_machine::STATUS_SUCCESS);
    assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
    assert!(app
        .config
        .targets
        .iter()
        .find(|target| target.target_id == "default")
        .unwrap()
        .root_path
        .join("process-note.md")
        .exists());
}

#[test]
fn import_without_process_leaves_work_queued() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let source = temp.path().join("queued-note.md");
    fs::write(&source, "queue me").unwrap();

    handle_import_command(&app, None, false, false, false, vec![source]).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();

    assert_eq!(batch.status, state_machine::STATUS_QUEUED);
    assert_eq!(items[0].status, state_machine::STATUS_QUEUED);
}

#[test]
fn import_process_failure_before_enqueue_does_not_drain_existing_queue() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let existing = temp.path().join("existing.md");
    let missing = temp.path().join("missing.md");
    fs::write(&existing, "still queued").unwrap();
    handle_import(&app, None, vec![existing]).unwrap();

    let err = handle_import_command(&app, None, true, false, false, vec![missing]).unwrap_err();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
    assert!(err.to_string().contains("failed to scan path"));
    assert_eq!(batch.status, state_machine::STATUS_QUEUED);
    assert_eq!(items[0].status, state_machine::STATUS_QUEUED);
}

#[test]
fn jobs_retry_requeues_failed_items_for_successful_agent_drain() {
    let temp = tempfile::tempdir().unwrap();
    let app = bootstrap_temp_app(&temp);
    let source = temp.path().join("retry-note.md");
    fs::write(&source, "will disappear").unwrap();
    handle_import(&app, None, vec![source.clone()]).unwrap();
    fs::remove_file(&source).unwrap();
    drain_queue(&app).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(10).unwrap().pop().unwrap();
    let failed_item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(failed_item.status, state_machine::STATUS_FAILED);
    drop(conn);

    fs::write(&source, "now exists").unwrap();
    assert_eq!(
        handle_jobs(
            &app,
            JobCommands::Retry {
                batch_id: batch.batch_id.clone(),
            },
        )
        .unwrap(),
        kbintake::exit_codes::SUCCESS
    );
    drain_queue(&app).unwrap();

    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.get_batch(&batch.batch_id).unwrap();
    let items = repo.list_items_by_batch(&batch.batch_id).unwrap();

    assert_eq!(batch.status, state_machine::STATUS_SUCCESS);
    assert_eq!(items[0].status, state_machine::STATUS_SUCCESS);
    assert_eq!(
        event_types_for_entity(&conn, "item", &items[0].item_id),
        vec![
            "item.queued",
            "item.failed",
            "item.retry_queued",
            "item.success"
        ]
    );
}

#[test]
fn jobs_undo_deletes_imported_files_and_marks_batch_undone() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("undo-note.md");
    fs::write(&source, "undo me").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());

    let app = App::bootstrap_in(app_data_dir.clone()).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(1).unwrap().pop().unwrap();
    let item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    let stored_path = item.stored_path.clone().unwrap();
    drop(conn);

    let output = kbintake_command(&app_data_dir)
        .args(["jobs", "undo", &batch.batch_id])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(kbintake::exit_codes::SUCCESS));
    assert!(!std::path::Path::new(&stored_path).exists());

    let app = App::bootstrap_in(app_data_dir).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.get_batch(&batch.batch_id).unwrap();
    let item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(batch.status, state_machine::STATUS_UNDONE);
    assert_eq!(item.status, state_machine::STATUS_UNDONE);
    assert_eq!(manifest_count_for_item(&conn, &item.item_id), 0);
}

#[test]
fn jobs_undo_returns_partial_when_file_modified() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("undo-modified.md");
    fs::write(&source, "original").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());

    let app = App::bootstrap_in(app_data_dir.clone()).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(1).unwrap().pop().unwrap();
    let item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    let stored_path = item.stored_path.clone().unwrap();
    drop(conn);
    fs::write(&stored_path, "changed").unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["jobs", "undo", &batch.batch_id])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::PARTIAL_SUCCESS)
    );
    assert!(std::path::Path::new(&stored_path).exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("WARN: File"));

    let app = App::bootstrap_in(app_data_dir).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.get_batch(&batch.batch_id).unwrap();
    let item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(batch.status, state_machine::STATUS_PARTIALLY_UNDONE);
    assert_eq!(item.status, state_machine::STATUS_UNDO_SKIPPED_MODIFIED);
    assert_eq!(manifest_count_for_item(&conn, &item.item_id), 1);
}

#[test]
fn jobs_undo_force_deletes_modified_file() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("undo-force.md");
    fs::write(&source, "original").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());

    let app = App::bootstrap_in(app_data_dir.clone()).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.list_batches(1).unwrap().pop().unwrap();
    let item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    let stored_path = item.stored_path.clone().unwrap();
    drop(conn);
    fs::write(&stored_path, "changed").unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["jobs", "undo", &batch.batch_id, "--force"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(kbintake::exit_codes::SUCCESS));
    assert!(!std::path::Path::new(&stored_path).exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("WARN: File"));

    let app = App::bootstrap_in(app_data_dir).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch = repo.get_batch(&batch.batch_id).unwrap();
    let item = repo
        .list_items_by_batch(&batch.batch_id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(batch.status, state_machine::STATUS_UNDONE);
    assert_eq!(item.status, state_machine::STATUS_UNDONE);
    assert_eq!(manifest_count_for_item(&conn, &item.item_id), 0);
}

#[test]
fn cli_jobs_list_json_supports_status_filter_and_limit() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let app = App::bootstrap_in(app_data_dir.clone()).unwrap();
    let source = temp.path().join("failed.md");
    fs::write(&source, "will fail").unwrap();
    handle_import(&app, None, vec![source.clone()]).unwrap();
    fs::remove_file(&source).unwrap();
    drain_queue(&app).unwrap();

    let output = kbintake_command(&app_data_dir)
        .args([
            "jobs", "list", "--status", "failed", "--limit", "1", "--json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let rows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["status"], state_machine::STATUS_FAILED);
}

#[test]
fn cli_jobs_show_json_returns_batch_and_items() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("show.md");
    fs::write(&source, "show me").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());

    let app = App::bootstrap_in(app_data_dir.clone()).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    let batch_id = repo.list_batches(1).unwrap().pop().unwrap().batch_id;
    drop(conn);

    let output = kbintake_command(&app_data_dir)
        .args(["jobs", "show", &batch_id, "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["batch_id"], batch_id);
    assert!(body["items"].as_array().is_some());
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["status"], state_machine::STATUS_SUCCESS);
}

#[test]
fn cli_jobs_list_rejects_unknown_status_filter() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");

    let output = kbintake_command(&app_data_dir)
        .args(["jobs", "list", "--status", "bad-status"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::INVALID_ARGUMENTS)
    );
}

#[test]
fn cli_returns_target_not_found_exit_code_for_invalid_import_target() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("note.md");
    fs::write(&source, "hello").unwrap();

    let output = kbintake_command(&temp.path().join("appdata"))
        .args(["import", "--target", "missing"])
        .arg(&source)
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::TARGET_NOT_FOUND)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("ERROR [4]:"));
}

#[test]
fn cli_returns_file_size_exceeded_when_all_processed_items_exceed_limit() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    assert!(kbintake_command(&app_data_dir)
        .arg("doctor")
        .output()
        .unwrap()
        .status
        .success());
    let config_path = app_data_dir.join("config.toml");
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replace("max_file_size_mb = 512", "max_file_size_mb = 0");
    fs::write(&config_path, config).unwrap();
    let source = temp.path().join("large.md");
    fs::write(&source, "too large").unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&source)
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::FILE_SIZE_EXCEEDED)
    );
}

#[test]
fn cli_returns_partial_success_when_processed_batch_has_success_and_failure() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    assert!(kbintake_command(&app_data_dir)
        .arg("doctor")
        .output()
        .unwrap()
        .status
        .success());
    let config_path = app_data_dir.join("config.toml");
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replace("max_file_size_mb = 512", "max_file_size_mb = 0");
    fs::write(&config_path, config).unwrap();
    let empty = temp.path().join("empty.md");
    let large = temp.path().join("large.md");
    fs::write(&empty, "").unwrap();
    fs::write(&large, "too large").unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&empty)
        .arg(&large)
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::PARTIAL_SUCCESS)
    );
}

#[test]
fn cli_returns_database_error_when_sqlite_file_cannot_be_opened() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    assert!(kbintake_command(&app_data_dir)
        .arg("doctor")
        .output()
        .unwrap()
        .status
        .success());
    let db_path = app_data_dir.join("data").join("kbintake.db");
    fs::remove_file(&db_path).unwrap();
    fs::create_dir(&db_path).unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["jobs", "list"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::DATABASE_ERROR)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("ERROR [8]:"));
}

#[test]
fn cli_import_dry_run_prints_preview_without_creating_batch() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("preview.md");
    fs::write(&source, "preview").unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["import", "--dry-run"])
        .arg(&source)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Source Path"));
    assert!(stdout.contains("Destination"));
    assert!(stdout.contains("copy"));

    let app = App::bootstrap_in(app_data_dir).unwrap();
    let conn = app.open_conn().unwrap();
    let repo = Repository::new(&conn);
    assert!(repo.list_batches(10).unwrap().is_empty());
    assert!(!app.config.targets[0].root_path.join("preview.md").exists());
}

#[test]
fn cli_import_dry_run_json_outputs_preview_array() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("preview.md");
    fs::write(&source, "preview").unwrap();

    let output = kbintake_command(&app_data_dir)
        .args(["import", "--dry-run", "--json"])
        .arg(&source)
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["action"], "copy");
    assert_eq!(value[0]["source"], source.display().to_string());
    assert!(value[0]["destination"]
        .as_str()
        .unwrap()
        .ends_with("preview.md"));
}

#[test]
fn cli_targets_list_hides_archived_targets_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let archive_path = temp.path().join("archive");
    fs::create_dir(&archive_path).unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["targets", "add", "archive"])
        .arg(&archive_path)
        .output()
        .unwrap()
        .status
        .success());
    assert!(kbintake_command(&app_data_dir)
        .args(["targets", "remove", "archive"])
        .output()
        .unwrap()
        .status
        .success());

    let active = kbintake_command(&app_data_dir)
        .args(["targets", "list"])
        .output()
        .unwrap();
    let all = kbintake_command(&app_data_dir)
        .args(["targets", "list", "--include-archived"])
        .output()
        .unwrap();

    assert!(!String::from_utf8_lossy(&active.stdout).contains("archive"));
    assert!(String::from_utf8_lossy(&all.stdout).contains("archived"));
}

#[test]
fn cli_targets_remove_pending_jobs_returns_operation_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let archive_path = temp.path().join("archive");
    fs::create_dir(&archive_path).unwrap();
    let source = temp.path().join("note.md");
    fs::write(&source, "hello").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["targets", "add", "archive"])
        .arg(&archive_path)
        .output()
        .unwrap()
        .status
        .success());
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--target", "archive"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());

    let output = kbintake_command(&app_data_dir)
        .args(["targets", "remove", "archive"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(kbintake::exit_codes::OPERATION_REJECTED)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("ERROR [5]:"));
}

#[test]
fn cli_vault_stats_json_empty_on_fresh_install() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");

    let output = kbintake_command(&app_data_dir)
        .args(["vault", "stats", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.is_array());
    assert_eq!(value.as_array().unwrap().len(), 1);
    assert_eq!(value[0]["files_imported"], 0);
    assert_eq!(value[0]["failed_count"], 0);
}

#[test]
fn cli_vault_stats_json_single_target_counts_imports() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let source = temp.path().join("note.md");
    fs::write(&source, "hello").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&source)
        .output()
        .unwrap()
        .status
        .success());

    let output = kbintake_command(&app_data_dir)
        .args(["vault", "stats", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["files_imported"], 1);
    assert_eq!(value[0]["failed_count"], 0);
    assert_eq!(value[0]["duplicate_count"], 0);
}

#[test]
fn cli_vault_stats_json_multiple_targets() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let default_source = temp.path().join("default.md");
    let archive_source = temp.path().join("archive.md");
    let archive_path = temp.path().join("archive");
    fs::create_dir(&archive_path).unwrap();
    fs::write(&default_source, "hello default").unwrap();
    fs::write(&archive_source, "hello archive").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["targets", "add", "archive"])
        .arg(&archive_path)
        .output()
        .unwrap()
        .status
        .success());
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&default_source)
        .output()
        .unwrap()
        .status
        .success());
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--target", "archive", "--process"])
        .arg(&archive_source)
        .output()
        .unwrap()
        .status
        .success());

    let output = kbintake_command(&app_data_dir)
        .args(["vault", "stats", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let default = rows
        .iter()
        .find(|row| row["target_id"] == "default")
        .unwrap();
    let archive = rows
        .iter()
        .find(|row| row["target_id"] == "archive")
        .unwrap();
    assert_eq!(default["files_imported"], 1);
    assert_eq!(archive["files_imported"], 1);
    assert_eq!(default["failed_count"], 0);
    assert_eq!(archive["failed_count"], 0);
}

#[test]
fn cli_vault_stats_json_target_filter_returns_single_row() {
    let temp = tempfile::tempdir().unwrap();
    let app_data_dir = temp.path().join("appdata");
    let default_source = temp.path().join("default.md");
    let archive_source = temp.path().join("archive.md");
    let archive_path = temp.path().join("archive");
    fs::create_dir(&archive_path).unwrap();
    fs::write(&default_source, "hello default").unwrap();
    fs::write(&archive_source, "hello archive").unwrap();
    assert!(kbintake_command(&app_data_dir)
        .args(["targets", "add", "archive"])
        .arg(&archive_path)
        .output()
        .unwrap()
        .status
        .success());
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--process"])
        .arg(&default_source)
        .output()
        .unwrap()
        .status
        .success());
    assert!(kbintake_command(&app_data_dir)
        .args(["import", "--target", "archive", "--process"])
        .arg(&archive_source)
        .output()
        .unwrap()
        .status
        .success());

    let output = kbintake_command(&app_data_dir)
        .args(["vault", "stats", "--target", "archive", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["target_id"], "archive");
    assert_eq!(rows[0]["files_imported"], 1);
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

fn event_types_for_entity(conn: &Connection, entity_type: &str, entity_id: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT event_type FROM events
             WHERE entity_type = ?1 AND entity_id = ?2
             ORDER BY created_at ASC",
        )
        .unwrap();
    stmt.query_map(params![entity_type, entity_id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}
