use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use crate::agent::scheduler;
use crate::app::App;
use crate::config::{self, AppConfig};
use crate::domain::{BatchJob, DomainEvent, ItemJob};
use crate::exit_codes;
use crate::processor::scanner;
use crate::queue::repository::Repository;

#[derive(Parser, Debug)]
#[command(name = "kbintake")]
#[command(about = "Windows knowledge-base intake agent")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Agent,
    Import {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        process: bool,
        paths: Vec<PathBuf>,
    },
    Jobs {
        #[command(subcommand)]
        command: JobCommands,
    },
    Targets {
        #[command(subcommand)]
        command: TargetCommands,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Explorer {
        #[command(subcommand)]
        command: ExplorerCommands,
    },
    Doctor,
    ConfigShow,
}

#[derive(Subcommand, Debug)]
pub enum JobCommands {
    List,
    Show { batch_id: String },
    Retry { batch_id: String },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    Show,
    SetTarget {
        path: PathBuf,
        #[arg(long, default_value = "default")]
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum TargetCommands {
    #[command(about = "List configured vault targets")]
    List,
    #[command(about = "Show one configured vault target")]
    Show {
        #[arg(help = "Target ID or name")]
        target: String,
    },
    #[command(about = "Add a vault target")]
    Add {
        #[arg(help = "Target name and ID")]
        name: String,
        #[arg(help = "Vault directory path")]
        path: PathBuf,
    },
    #[command(about = "Rename a vault target")]
    Rename {
        #[arg(help = "Current target ID or name")]
        target: String,
        #[arg(help = "New target name and ID")]
        new_name: String,
    },
    #[command(about = "Remove a vault target")]
    Remove {
        #[arg(help = "Target ID or name")]
        target: String,
    },
    #[command(about = "Make a target the default import target")]
    SetDefault {
        #[arg(help = "Target ID or name")]
        target: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ExplorerCommands {
    #[command(about = "Register Windows Explorer right-click menu entries")]
    Install {
        #[arg(
            long,
            help = "Executable path to register; defaults to the current exe"
        )]
        exe_path: Option<PathBuf>,
        #[arg(
            long,
            help = "Icon path to register; defaults to kbintake.ico next to the exe"
        )]
        icon_path: Option<PathBuf>,
        #[arg(long, help = "Queue right-click imports without immediate processing")]
        queue_only: bool,
    },
    #[command(about = "Remove Windows Explorer right-click menu entries")]
    Uninstall,
}

#[derive(Debug, Clone)]
pub struct ImportOutcome {
    pub batch_id: String,
    pub item_count: usize,
    pub target_name: String,
}

pub fn handle_import_command(
    app: &App,
    target_id: Option<String>,
    process: bool,
    paths: Vec<PathBuf>,
) -> Result<i32> {
    let outcome = handle_import(app, target_id, paths)?;
    if process {
        scheduler::drain_queue(app)?;
        return import_exit_code(app, &outcome.batch_id);
    }
    Ok(exit_codes::SUCCESS)
}

pub fn handle_import(
    app: &App,
    target_id: Option<String>,
    paths: Vec<PathBuf>,
) -> Result<ImportOutcome> {
    if paths.is_empty() {
        anyhow::bail!("no input paths provided");
    }

    let target = match target_id {
        Some(target_id) => app.config.target_by_id(&target_id)?,
        None => app.config.default_target()?,
    };
    let mut files = Vec::new();
    for path in paths {
        let discovered = scanner::expand_input_path(&path)
            .with_context(|| format!("failed to scan path {}", path.display()))?;
        files.extend(discovered);
    }
    if files.is_empty() {
        anyhow::bail!("no importable files found");
    }

    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let batch = BatchJob::new("cli", &target.target_id, files.len() as i64);
    repo.insert_batch(&batch)?;
    repo.insert_event(&DomainEvent::new(
        "batch",
        batch.batch_id.clone(),
        "batch.queued",
        serde_json::json!({
            "source": batch.source,
            "target_id": batch.target_id,
            "source_count": batch.source_count
        }),
    ))?;

    let mut count = 0usize;
    for file in files {
        let item = ItemJob::new(batch.batch_id.clone(), target.target_id.clone(), file);
        repo.insert_item(&item)?;
        repo.insert_event(&DomainEvent::new(
            "item",
            item.item_id.clone(),
            "item.queued",
            serde_json::json!({
                "batch_id": item.batch_id,
                "target_id": item.target_id,
                "source_path": item.source_path
            }),
        ))?;
        count += 1;
    }

    info!(batch_id = %batch.batch_id, items = count, "batch queued");
    println!("Queued batch: {}", batch.batch_id);
    println!("Items queued: {}", count);
    println!("Target: {}", target.name);
    Ok(ImportOutcome {
        batch_id: batch.batch_id,
        item_count: count,
        target_name: target.name,
    })
}

pub fn handle_jobs(app: &App, command: JobCommands) -> Result<()> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);

    match command {
        JobCommands::List => {
            for row in repo.list_batches(20)? {
                println!(
                    "{}  {}  items={}  target={}  created={}",
                    row.batch_id, row.status, row.source_count, row.target_id, row.created_at
                );
            }
        }
        JobCommands::Show { batch_id } => {
            let batch = repo.get_batch(&batch_id)?;
            let items = repo.list_items_by_batch(&batch_id)?;
            println!("Batch: {}", batch.batch_id);
            println!("Status: {}", batch.status);
            println!("Items: {}", items.len());
            for item in items {
                println!("{}", format_item_line(item));
            }
            print_events(&repo, "batch", &batch.batch_id)?;
            for item in repo.list_items_by_batch(&batch_id)? {
                print_events(&repo, "item", &item.item_id)?;
            }
        }
        JobCommands::Retry { batch_id } => {
            let failed_items = repo
                .list_items_by_batch(&batch_id)?
                .into_iter()
                .filter(|item| item.status == crate::queue::state_machine::STATUS_FAILED)
                .collect::<Vec<_>>();
            let retried = repo.retry_failed_items_by_batch(&batch_id)?;
            for item in failed_items {
                repo.insert_event(&DomainEvent::new(
                    "item",
                    item.item_id,
                    "item.retry_queued",
                    serde_json::json!({
                        "batch_id": batch_id,
                        "status": "queued"
                    }),
                ))?;
            }
            println!("Retried items: {retried}");
        }
    }
    Ok(())
}

fn print_events(repo: &Repository<'_>, entity_type: &str, entity_id: &str) -> Result<()> {
    for event in repo.list_events_for_entity(entity_type, entity_id)? {
        println!("{}", format_event_line(event));
    }
    Ok(())
}

fn format_event_line(event: DomainEvent) -> String {
    format!(
        "event {} {} {}",
        event.created_at, event.event_type, event.payload_json
    )
}

fn format_item_line(item: ItemJob) -> String {
    format!(
        "- {} [{}] target={} {} -> {}{}{}{}",
        item.item_id,
        item.status,
        item.target_id,
        item.source_path,
        item.stored_path.unwrap_or_else(|| "-".to_string()),
        item.stage
            .map(|stage| format!(" stage={stage}"))
            .unwrap_or_default(),
        item.duplicate_of
            .map(|duplicate_of| format!(" duplicate_of={duplicate_of}"))
            .unwrap_or_default(),
        match (item.error_code, item.error_message) {
            (Some(code), Some(message)) => format!(" error={code}: {message}"),
            (Some(code), None) => format!(" error={code}"),
            _ => String::new(),
        }
    )
}

pub fn handle_doctor(app: &App) -> Result<()> {
    let target = app.config.default_target()?;
    let conn = app.open_conn()?;
    crate::db::validate_schema(&conn)?;
    config::validate_target_root(&target.root_path)?;
    println!("Config dir: {}", app.config.app_data_dir.display());
    println!("Database: {}", app.db_path.display());
    println!(
        "Default target: {} ({})",
        target.name,
        target.root_path.display()
    );
    println!("Schema: ok");
    println!("Target: ok");
    Ok(())
}

pub fn handle_config_show(app: &App) -> Result<()> {
    println!("{}", toml::to_string_pretty(&app.config)?);
    Ok(())
}

pub fn handle_config(app: &App, command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show => handle_config_show(app),
        ConfigCommands::SetTarget { path, name } => {
            let mut config = AppConfig::load_or_init_in(app.config.app_data_dir.clone())?;
            let target = config.set_default_target(name, path)?;
            config::validate_target_root(&target.root_path)?;
            config.save()?;
            println!("Default target: {}", target.name);
            println!("Path: {}", target.root_path.display());
            Ok(())
        }
    }
}

pub fn handle_targets(app: &App, command: TargetCommands) -> Result<()> {
    match command {
        TargetCommands::List => {
            for (index, target) in app.config.targets.iter().enumerate() {
                let marker = if index == 0 { "*" } else { " " };
                println!(
                    "{marker} {}  {}  {}",
                    target.target_id,
                    target.name,
                    target.root_path.display()
                );
            }
            Ok(())
        }
        TargetCommands::Show { target } => {
            let target = app.config.target_by_id(&target)?;
            println!("Target: {}", target.target_id);
            println!("Name: {}", target.name);
            println!("Path: {}", target.root_path.display());
            Ok(())
        }
        TargetCommands::Add { name, path } => {
            let mut config = AppConfig::load_or_init_in(app.config.app_data_dir.clone())?;
            let target = config.add_target(name, path)?;
            config::validate_target_root(&target.root_path)?;
            config.save()?;
            println!("Added target: {}", target.target_id);
            println!("Path: {}", target.root_path.display());
            Ok(())
        }
        TargetCommands::Rename { target, new_name } => {
            let mut config = AppConfig::load_or_init_in(app.config.app_data_dir.clone())?;
            let target = config.rename_target(&target, new_name)?;
            config.save()?;
            println!("Renamed target: {}", target.target_id);
            println!("Path: {}", target.root_path.display());
            Ok(())
        }
        TargetCommands::Remove { target } => {
            let mut config = AppConfig::load_or_init_in(app.config.app_data_dir.clone())?;
            let removed = config.remove_target(&target)?;
            config.save()?;
            println!("Removed target: {}", removed.target_id);
            if let Ok(default_target) = config.default_target() {
                println!("Default target: {}", default_target.target_id);
            }
            Ok(())
        }
        TargetCommands::SetDefault { target } => {
            let mut config = AppConfig::load_or_init_in(app.config.app_data_dir.clone())?;
            let target = config.set_default_target_by_id(&target)?;
            config.save()?;
            println!("Default target: {}", target.target_id);
            println!("Path: {}", target.root_path.display());
            Ok(())
        }
    }
}

fn import_exit_code(app: &App, batch_id: &str) -> Result<i32> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let items = repo.list_items_by_batch(batch_id)?;
    let failed_items = items
        .iter()
        .filter(|item| item.status == crate::queue::state_machine::STATUS_FAILED)
        .collect::<Vec<_>>();

    if failed_items.is_empty() {
        return Ok(exit_codes::SUCCESS);
    }

    if failed_items.len() < items.len() {
        return Ok(exit_codes::PARTIAL_SUCCESS);
    }

    if failed_items.iter().all(|item| {
        item.error_message
            .as_deref()
            .is_some_and(|message| message.contains("exceeds max size"))
    }) {
        return Ok(exit_codes::FILE_SIZE_EXCEEDED);
    }

    Ok(exit_codes::GENERAL_ERROR)
}

pub fn handle_explorer(command: ExplorerCommands) -> Result<()> {
    match command {
        ExplorerCommands::Install {
            exe_path,
            icon_path,
            queue_only,
        } => {
            let mut options = crate::explorer::default_install_options(queue_only)?;
            if let Some(exe_path) = exe_path {
                options.exe_path = exe_path;
                if icon_path.is_none() {
                    options.icon_path =
                        crate::explorer::discover_icon_next_to_exe(&options.exe_path);
                }
            }
            if icon_path.is_some() {
                options.icon_path = icon_path;
            }

            let registrations = crate::explorer::install(&options)?;
            for registration in registrations {
                println!("Registered: HKCU\\{}", registration.menu_key);
                println!("Command: {}", registration.command);
                if let Some(icon_path) = registration.icon_path {
                    println!("Icon: {}", icon_path.display());
                }
            }
            Ok(())
        }
        ExplorerCommands::Uninstall => {
            crate::explorer::uninstall()?;
            println!("Removed Explorer context-menu entries");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;

    use super::{
        format_event_line, format_item_line, handle_config, handle_import, handle_import_command,
        handle_targets, ConfigCommands, TargetCommands,
    };
    use crate::app::App;
    use crate::config::{AppConfig, ImportConfig};
    use crate::db;
    use crate::domain::{DomainEvent, ItemJob, Target};
    use crate::exit_codes;
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
                },
            },
            db_path,
        }
    }

    #[test]
    fn import_rejects_missing_later_path_without_partial_batch() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let valid = temp.path().join("valid.md");
        let missing = temp.path().join("missing.md");
        fs::write(&valid, "hello").unwrap();

        let err = handle_import(&app, None, vec![valid, missing]).unwrap_err();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        assert!(err.to_string().contains("failed to scan path"));
        assert!(repo.list_batches(20).unwrap().is_empty());
    }

    #[test]
    fn import_rejects_directory_with_no_files() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let empty_dir = temp.path().join("empty");
        fs::create_dir(&empty_dir).unwrap();

        let err = handle_import(&app, None, vec![empty_dir]).unwrap_err();

        assert!(err.to_string().contains("no importable files found"));
    }

    #[test]
    fn config_set_target_persists_default_target() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let target_root = temp.path().join("new-vault");

        handle_config(
            &app,
            ConfigCommands::SetTarget {
                path: target_root.clone(),
                name: "archive".to_string(),
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(reloaded.config.targets[0].name, "archive");
        assert_eq!(reloaded.config.targets[0].root_path, target_root);
        assert!(reloaded.config.targets[0].root_path.is_dir());
    }

    #[test]
    fn import_after_config_set_target_uses_new_target() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let target_root = temp.path().join("new-vault");
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();
        handle_config(
            &app,
            ConfigCommands::SetTarget {
                path: target_root,
                name: "archive".to_string(),
            },
        )
        .unwrap();
        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();

        let outcome = handle_import(&reloaded, None, vec![source]).unwrap();

        let conn = reloaded.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.list_batches(1).unwrap().pop().unwrap();
        let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
        let batch_events = repo
            .list_events_for_entity("batch", &batch.batch_id)
            .unwrap();
        let item_events = repo
            .list_events_for_entity("item", &items[0].item_id)
            .unwrap();
        assert_eq!(batch.target_id, "archive");
        assert_eq!(items[0].target_id, "archive");
        assert_eq!(batch_events[0].event_type, "batch.queued");
        assert_eq!(item_events[0].event_type, "item.queued");
        assert_eq!(outcome.batch_id, batch.batch_id);
        assert_eq!(outcome.item_count, 1);
        assert_eq!(outcome.target_name, "archive");
    }

    #[test]
    fn targets_add_persists_additional_target() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let target_root = temp.path().join("archive");

        handle_targets(
            &app,
            TargetCommands::Add {
                name: "archive".to_string(),
                path: target_root.clone(),
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(reloaded.config.targets.len(), 2);
        assert_eq!(reloaded.config.targets[0].target_id, "default");
        assert_eq!(reloaded.config.targets[1].target_id, "archive");
        assert_eq!(reloaded.config.targets[1].root_path, target_root);
    }

    #[test]
    fn targets_rename_persists_updated_target_name() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        handle_targets(
            &app,
            TargetCommands::Add {
                name: "archive".to_string(),
                path: temp.path().join("archive"),
            },
        )
        .unwrap();
        let app = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();

        handle_targets(
            &app,
            TargetCommands::Rename {
                target: "archive".to_string(),
                new_name: "notes".to_string(),
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert!(reloaded.config.target_by_id("archive").is_err());
        assert_eq!(reloaded.config.target_by_id("notes").unwrap().name, "notes");
    }

    #[test]
    fn targets_remove_persists_target_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        handle_targets(
            &app,
            TargetCommands::Add {
                name: "archive".to_string(),
                path: temp.path().join("archive"),
            },
        )
        .unwrap();
        let app = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();

        handle_targets(
            &app,
            TargetCommands::Remove {
                target: "archive".to_string(),
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(reloaded.config.targets.len(), 1);
        assert_eq!(reloaded.config.targets[0].target_id, "default");
        assert!(reloaded.config.target_by_id("archive").is_err());
    }

    #[test]
    fn targets_set_default_changes_default_import_target() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        handle_targets(
            &app,
            TargetCommands::Add {
                name: "archive".to_string(),
                path: temp.path().join("archive"),
            },
        )
        .unwrap();
        let app = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();

        handle_targets(
            &app,
            TargetCommands::SetDefault {
                target: "archive".to_string(),
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(reloaded.config.targets[0].target_id, "archive");
    }

    #[test]
    fn import_with_explicit_target_queues_for_that_target() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        handle_targets(
            &app,
            TargetCommands::Add {
                name: "archive".to_string(),
                path: temp.path().join("archive"),
            },
        )
        .unwrap();
        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();

        handle_import(&reloaded, Some("archive".to_string()), vec![source]).unwrap();

        let conn = reloaded.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.list_batches(1).unwrap().pop().unwrap();
        let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
        assert_eq!(batch.target_id, "archive");
        assert_eq!(items[0].target_id, "archive");
    }

    #[test]
    fn import_with_unknown_target_fails_before_creating_batch() {
        let temp = tempfile::tempdir().unwrap();
        let app = test_app(&temp);
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();

        let err = handle_import(&app, Some("missing".to_string()), vec![source]).unwrap_err();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        assert!(err.to_string().contains("target not configured"));
        assert!(repo.list_batches(20).unwrap().is_empty());
    }

    #[test]
    fn import_process_returns_partial_success_when_some_items_fail() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config.import.max_file_size_mb = 0;
        let empty = temp.path().join("empty.md");
        let large = temp.path().join("large.md");
        fs::write(&empty, "").unwrap();
        fs::write(&large, "too large").unwrap();

        let code = handle_import_command(&app, None, true, vec![empty, large]).unwrap();

        assert_eq!(code, exit_codes::PARTIAL_SUCCESS);
    }

    #[test]
    fn import_process_returns_file_size_exceeded_when_all_items_exceed_limit() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config.import.max_file_size_mb = 0;
        let large = temp.path().join("large.md");
        fs::write(&large, "too large").unwrap();

        let code = handle_import_command(&app, None, true, vec![large]).unwrap();

        assert_eq!(code, exit_codes::FILE_SIZE_EXCEEDED);
    }

    #[test]
    fn jobs_item_line_includes_target_and_failure_details() {
        let mut item = ItemJob::new(
            "batch".to_string(),
            "archive".to_string(),
            "source.md".into(),
        );
        item.status = "failed".to_string();
        item.error_code = Some("E_TEST".to_string());
        item.error_message = Some("test failure".to_string());

        let line = format_item_line(item);

        assert!(line.contains("target=archive"));
        assert!(line.contains("error=E_TEST: test failure"));
    }

    #[test]
    fn jobs_event_line_includes_type_and_payload() {
        let event = DomainEvent::new(
            "item",
            "item-1",
            "item.failed",
            serde_json::json!({ "error_code": "E_TEST" }),
        );

        let line = format_event_line(event);

        assert!(line.contains("item.failed"));
        assert!(line.contains("\"error_code\":\"E_TEST\""));
    }
}
