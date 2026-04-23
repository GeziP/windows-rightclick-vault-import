use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
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
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
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
    Vault {
        #[command(subcommand)]
        command: VaultCommands,
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
    List {
        #[arg(long, help = "Include archived targets")]
        include_archived: bool,
    },
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
        #[arg(long, help = "Archive the target even if queued items exist")]
        force: bool,
    },
    #[command(about = "Make a target the default import target")]
    SetDefault {
        #[arg(help = "Target ID or name")]
        target: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum VaultCommands {
    #[command(about = "Show per-target vault import stats")]
    Stats {
        #[arg(long, help = "Output stats as JSON")]
        json: bool,
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
    dry_run: bool,
    json: bool,
    paths: Vec<PathBuf>,
) -> Result<i32> {
    if dry_run {
        let rows = crate::processor::dry_run::preview_import(app, target_id, paths)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&rows)?);
        } else {
            crate::processor::dry_run::print_table(&rows);
        }
        return Ok(exit_codes::SUCCESS);
    }

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
        TargetCommands::List { include_archived } => {
            if include_archived {
                println!("default  target  name  status  path");
            }
            for (index, target) in app
                .config
                .targets
                .iter()
                .enumerate()
                .filter(|(_, target)| include_archived || target.is_active())
            {
                let marker = if index == 0 && target.is_active() {
                    "*"
                } else {
                    " "
                };
                if include_archived {
                    println!(
                        "{marker}  {}  {}  {}  {}",
                        target.target_id,
                        target.name,
                        target.status,
                        target.root_path.display()
                    );
                } else {
                    println!(
                        "{marker} {}  {}  {}",
                        target.target_id,
                        target.name,
                        target.root_path.display()
                    );
                }
            }
            Ok(())
        }
        TargetCommands::Show { target } => {
            let target = app.config.target_any_by_id(&target)?;
            println!("Target: {}", target.target_id);
            println!("Name: {}", target.name);
            println!("Status: {}", target.status);
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
        TargetCommands::Remove { target, force } => {
            let mut config = AppConfig::load_or_init_in(app.config.app_data_dir.clone())?;
            let target_to_remove = config.target_any_by_id(&target)?;
            let conn = app.open_conn()?;
            let repo = Repository::new(&conn);
            let queued = repo.count_queued_items_by_target(&target_to_remove.target_id)?;
            if queued > 0 && !force {
                anyhow::bail!(
                    "Cannot remove target '{}' - {} pending job(s) exist. Process or cancel them first.",
                    target_to_remove.name,
                    queued
                );
            }
            if queued > 0 {
                eprintln!(
                    "WARN: Target '{}' had {} queued item(s) - forced archive.",
                    target_to_remove.name, queued
                );
            }
            let removed = config.remove_target(&target)?;
            config.save()?;
            println!("Archived target: {}", removed.target_id);
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

#[derive(Debug, Serialize)]
struct VaultStatsRow {
    target_id: String,
    name: String,
    root_path: String,
    is_default: bool,
    files_imported: i64,
    storage_bytes: i64,
    duplicate_count: i64,
    duplicate_percent: f64,
    failed_count: i64,
    last_import_at: Option<String>,
}

pub fn handle_vault(app: &App, command: VaultCommands) -> Result<()> {
    match command {
        VaultCommands::Stats { json } => handle_vault_stats(app, json),
    }
}

fn handle_vault_stats(app: &App, json: bool) -> Result<()> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let mut rows = Vec::new();
    for (index, target) in app
        .config
        .targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_active())
    {
        let stats = repo.target_stats(&target.target_id)?;
        let processed = stats.success_count + stats.duplicate_count;
        let duplicate_percent = if processed == 0 {
            0.0
        } else {
            (stats.duplicate_count as f64 * 100.0) / processed as f64
        };
        rows.push(VaultStatsRow {
            target_id: target.target_id.clone(),
            name: target.name.clone(),
            root_path: target.root_path.display().to_string(),
            is_default: index == 0,
            files_imported: stats.imported_files,
            storage_bytes: stats.storage_bytes,
            duplicate_count: stats.duplicate_count,
            duplicate_percent,
            failed_count: stats.failed_count,
            last_import_at: stats.last_import_at,
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("No active targets configured.");
        return Ok(());
    }

    for row in rows {
        let default_marker = if row.is_default { " (default)" } else { "" };
        println!("Target: {}{}", row.name, default_marker);
        println!("  Path:          {}", row.root_path);
        println!("  Files imported: {}", row.files_imported);
        println!("  Storage used:  {}", format_bytes(row.storage_bytes));
        println!(
            "  Duplicates:    {:.0}%  ({} skipped)",
            row.duplicate_percent, row.duplicate_count
        );
        println!("  Failed:        {}", row.failed_count);
        println!(
            "  Last import:   {}",
            row.last_import_at
                .as_deref()
                .map(format_timestamp)
                .unwrap_or("-".to_string())
        );
        println!();
    }
    Ok(())
}

fn format_bytes(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes.max(0))
    }
}

fn format_timestamp(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| value.to_string())
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
    fn targets_remove_archives_target() {
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
                force: false,
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(reloaded.config.targets.len(), 2);
        assert_eq!(reloaded.config.targets[0].target_id, "default");
        assert_eq!(
            reloaded.config.target_any_by_id("archive").unwrap().status,
            "archived"
        );
        assert!(reloaded.config.target_by_id("archive").is_err());
    }

    #[test]
    fn targets_remove_rejects_target_with_queued_items() {
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
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();
        handle_import(&app, Some("archive".to_string()), vec![source]).unwrap();

        let err = handle_targets(
            &app,
            TargetCommands::Remove {
                target: "archive".to_string(),
                force: false,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("pending job"));
        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(
            reloaded.config.target_by_id("archive").unwrap().status,
            "active"
        );
    }

    #[test]
    fn targets_remove_force_archives_target_with_queued_items() {
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
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();
        handle_import(&app, Some("archive".to_string()), vec![source]).unwrap();

        handle_targets(
            &app,
            TargetCommands::Remove {
                target: "archive".to_string(),
                force: true,
            },
        )
        .unwrap();

        let reloaded = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        assert_eq!(
            reloaded.config.target_any_by_id("archive").unwrap().status,
            "archived"
        );
    }

    #[test]
    fn archived_targets_reject_rename_set_default_and_import() {
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
                force: false,
            },
        )
        .unwrap();
        let app = App::bootstrap_in(app.config.app_data_dir.clone()).unwrap();
        let source = temp.path().join("note.md");
        fs::write(&source, "hello").unwrap();

        let rename_err = handle_targets(
            &app,
            TargetCommands::Rename {
                target: "archive".to_string(),
                new_name: "notes".to_string(),
            },
        )
        .unwrap_err();
        let default_err = handle_targets(
            &app,
            TargetCommands::SetDefault {
                target: "archive".to_string(),
            },
        )
        .unwrap_err();
        let import_err =
            handle_import(&app, Some("archive".to_string()), vec![source]).unwrap_err();

        assert!(rename_err.to_string().contains("archived"));
        assert!(default_err.to_string().contains("archived"));
        assert!(import_err.to_string().contains("archived"));
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

        let code =
            handle_import_command(&app, None, true, false, false, vec![empty, large]).unwrap();

        assert_eq!(code, exit_codes::PARTIAL_SUCCESS);
    }

    #[test]
    fn import_process_returns_file_size_exceeded_when_all_items_exceed_limit() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config.import.max_file_size_mb = 0;
        let large = temp.path().join("large.md");
        fs::write(&large, "too large").unwrap();

        let code = handle_import_command(&app, None, true, false, false, vec![large]).unwrap();

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
