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
use crate::notify::ToastContent;
use crate::processor::scanner;
use crate::queue::repository::Repository;

#[derive(Parser, Debug)]
#[command(name = "kbintake")]
#[command(about = "Windows knowledge-base intake agent")]
#[command(version)]
pub struct Cli {
    #[arg(long, global = true, hide = true)]
    pub app_data_dir: Option<PathBuf>,
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
    Service {
        #[command(subcommand)]
        command: ServiceCommands,
    },
    Doctor {
        #[arg(long, help = "Apply safe automatic fixes")]
        fix: bool,
        #[arg(
            long,
            help = "Apply pending database migrations before reporting status"
        )]
        migrate: bool,
    },
    ConfigShow,
    Version,
}

#[derive(Subcommand, Debug)]
pub enum JobCommands {
    List {
        #[arg(long, help = "Filter by batch status")]
        status: Option<String>,
        #[arg(long, default_value_t = 20, help = "Maximum batches to show")]
        limit: usize,
        #[arg(long, help = "Output as JSON")]
        json: bool,
        #[arg(long, help = "Output as table (default unless --json)")]
        table: bool,
    },
    Show {
        batch_id: String,
        #[arg(long, help = "Output as JSON")]
        json: bool,
        #[arg(long, help = "Output as table")]
        table: bool,
    },
    Retry {
        batch_id: String,
    },
    Undo {
        batch_id: String,
        #[arg(long, help = "Delete modified files during undo")]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    Show,
    Validate,
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
        #[arg(long, help = "Show stats for a single target ID or name")]
        target: Option<String>,
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
    #[command(hide = true)]
    ComFeasibility,
    #[command(hide = true)]
    RunImport {
        #[arg(long, help = "Queue right-click imports without immediate processing")]
        queue_only: bool,
        paths: Vec<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServiceCommands {
    Install,
    Start,
    Stop,
    Uninstall,
    Status,
    #[command(hide = true)]
    Run,
}

#[derive(Debug, Clone)]
pub struct ImportOutcome {
    pub batch_id: String,
    pub item_count: usize,
    pub target_name: String,
    pub routing_summary: RoutingSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerBatchSummary {
    pub batch_id: String,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
    pub target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingSummary {
    None,
    Single(String),
    Multiple,
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

    let explicit_target = match target_id {
        Some(target_id) => Some(app.config.target_by_id(&target_id)?),
        None => None,
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
    let routed_files = files
        .into_iter()
        .map(|file| {
            let source_size_bytes = std::fs::metadata(&file)
                .with_context(|| format!("failed to inspect {}", file.display()))?
                .len();
            let (target, matched_rule_template) = match &explicit_target {
                Some(target) => (target.clone(), None),
                None => {
                    let selection = app
                        .config
                        .route_selection_for_path(&file, source_size_bytes)?;
                    (selection.target, selection.matched_rule_template)
                }
            };
            Ok((file, target, matched_rule_template))
        })
        .collect::<Result<Vec<_>>>()?;
    let batch_target_id = common_target_id(&routed_files)
        .unwrap_or("mixed")
        .to_string();
    let routing_summary = summarize_routing(&routed_files);
    let batch = BatchJob::new("cli", &batch_target_id, routed_files.len() as i64);
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
    for (file, target, _) in routed_files {
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
    let target_name = if batch_target_id == "mixed" {
        "mixed".to_string()
    } else {
        app.config.target_any_by_id(&batch_target_id)?.name
    };
    println!("Target: {}", target_name);
    match &routing_summary {
        RoutingSummary::Single(rule) => println!("Routing rule: {}", rule),
        RoutingSummary::Multiple => println!("Routing rule: multiple"),
        RoutingSummary::None => {}
    }
    Ok(ImportOutcome {
        batch_id: batch.batch_id,
        item_count: count,
        target_name,
        routing_summary,
    })
}

fn common_target_id(
    routed_files: &[(PathBuf, crate::domain::Target, Option<String>)],
) -> Option<&str> {
    let first = routed_files.first()?.1.target_id.as_str();
    routed_files
        .iter()
        .all(|(_, target, _)| target.target_id == first)
        .then_some(first)
}

fn summarize_routing(
    routed_files: &[(PathBuf, crate::domain::Target, Option<String>)],
) -> RoutingSummary {
    let mut matched = routed_files
        .iter()
        .filter_map(|(_, _, template)| template.as_deref())
        .collect::<Vec<_>>();
    matched.sort_unstable();
    matched.dedup();
    match matched.as_slice() {
        [] => RoutingSummary::None,
        [single] => RoutingSummary::Single((*single).to_string()),
        _ => RoutingSummary::Multiple,
    }
}

pub fn handle_jobs(app: &App, command: JobCommands) -> Result<i32> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);

    match command {
        JobCommands::List {
            status,
            limit,
            json,
            table,
        } => {
            ensure_job_output_mode(json, table)?;
            let status = parse_batch_status_filter(status)?;
            let rows = repo.list_batches_filtered(limit as i64, status.as_deref())?;
            if json {
                let out = rows
                    .into_iter()
                    .map(|row| JobListRow {
                        batch_id: row.batch_id,
                        status: row.status,
                        source_count: row.source_count,
                        target_id: row.target_id,
                        created_at: row.created_at.to_rfc3339(),
                        updated_at: row.updated_at.to_rfc3339(),
                    })
                    .collect::<Vec<_>>();
                println!("{}", serde_json::to_string_pretty(&out)?);
                return Ok(exit_codes::SUCCESS);
            }

            println!("{}", format_job_list_header());
            for row in rows {
                println!("{}", format_job_list_row(&row));
            }
            Ok(exit_codes::SUCCESS)
        }
        JobCommands::Show {
            batch_id,
            json,
            table,
        } => {
            ensure_job_output_mode(json, table)?;
            let batch = repo.get_batch(&batch_id)?;
            let items = repo.list_items_by_batch(&batch_id)?;
            if json {
                let out = JobShowRow {
                    batch_id: batch.batch_id,
                    status: batch.status,
                    source: batch.source,
                    source_count: batch.source_count,
                    created_at: batch.created_at.to_rfc3339(),
                    updated_at: batch.updated_at.to_rfc3339(),
                    items: items
                        .into_iter()
                        .map(|item| JobShowItemRow {
                            item_id: item.item_id,
                            status: item.status,
                            target_id: item.target_id,
                            source_path: item.source_path,
                            stored_path: item.stored_path,
                            error_code: item.error_code,
                            error_message: item.error_message,
                        })
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&out)?);
                return Ok(exit_codes::SUCCESS);
            }

            println!("Batch: {}", batch.batch_id);
            println!("Status: {}", batch.status);
            println!("Source: {}", batch.source);
            println!("Target: {}", batch.target_id);
            println!("Items: {}", items.len());
            println!("{}", format_job_show_header());
            for item in items {
                println!("{}", format_job_show_row(&item));
            }
            print_events(&repo, "batch", &batch.batch_id)?;
            for item in repo.list_items_by_batch(&batch_id)? {
                print_events(&repo, "item", &item.item_id)?;
            }
            Ok(exit_codes::SUCCESS)
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
            Ok(exit_codes::SUCCESS)
        }
        JobCommands::Undo { batch_id, force } => {
            let batch = repo.get_batch(&batch_id)?;
            if batch.status == crate::queue::state_machine::STATUS_UNDONE
                || batch.status == crate::queue::state_machine::STATUS_PARTIALLY_UNDONE
            {
                println!("Batch already undone: {}", batch.batch_id);
                return Ok(exit_codes::SUCCESS);
            }
            if batch.status == crate::queue::state_machine::STATUS_QUEUED
                || batch.status == crate::queue::state_machine::STATUS_RUNNING
            {
                anyhow::bail!(
                    "Cannot undo batch '{}' while it is {}.",
                    batch.batch_id,
                    batch.status
                );
            }
            let items = repo.list_items_by_batch(&batch_id)?;
            let mut deleted_count = 0usize;
            let mut skipped_modified_count = 0usize;

            for item in items {
                if item.status != crate::queue::state_machine::STATUS_SUCCESS {
                    continue;
                }

                let Some(stored_path) = item.stored_path.clone() else {
                    let message =
                        format!("File path missing for item '{}' during undo.", item.item_id);
                    repo.mark_item_undo_skipped_modified(&item.item_id, &message)?;
                    repo.insert_event(&DomainEvent::new(
                        "item",
                        item.item_id.clone(),
                        "item.undo_skipped_modified",
                        serde_json::json!({
                            "status": crate::queue::state_machine::STATUS_UNDO_SKIPPED_MODIFIED,
                            "reason": "stored_path_missing",
                            "message": message
                        }),
                    ))?;
                    skipped_modified_count += 1;
                    continue;
                };

                let path = PathBuf::from(&stored_path);
                if path.exists() {
                    let hash_matches = if let Some(expected_hash) = item.stored_sha256.as_deref() {
                        crate::processor::hasher::sha256_file(&path)? == expected_hash
                    } else {
                        let expected_hash = item.sha256.as_deref().unwrap_or_default();
                        if app.config.import.inject_frontmatter
                            && crate::processor::frontmatter::is_markdown_extension(
                                item.file_ext.as_deref(),
                            )
                        {
                            crate::processor::frontmatter::file_matches_original_hash(
                                &path,
                                expected_hash,
                            )?
                        } else {
                            crate::processor::hasher::sha256_file(&path)? == expected_hash
                        }
                    };
                    if !hash_matches {
                        let warning = format!(
                            "File '{}' skipped during undo - content has been modified since import.",
                            stored_path
                        );
                        eprintln!("WARN: {warning}");
                        if !force {
                            repo.mark_item_undo_skipped_modified(&item.item_id, &warning)?;
                            repo.insert_event(&DomainEvent::new(
                                "item",
                                item.item_id.clone(),
                                "item.undo_skipped_modified",
                                serde_json::json!({
                                    "status": crate::queue::state_machine::STATUS_UNDO_SKIPPED_MODIFIED,
                                    "stored_path": stored_path,
                                    "force": false,
                                    "message": warning
                                }),
                            ))?;
                            skipped_modified_count += 1;
                            continue;
                        }
                    }

                    std::fs::remove_file(&path).with_context(|| {
                        format!("failed to remove imported file {}", path.display())
                    })?;
                }

                repo.delete_manifest_by_item(&item.item_id)?;
                repo.mark_item_undone(&item.item_id)?;
                repo.insert_event(&DomainEvent::new(
                    "item",
                    item.item_id.clone(),
                    "item.undone",
                    serde_json::json!({
                        "status": crate::queue::state_machine::STATUS_UNDONE,
                        "stored_path": stored_path,
                        "force": force
                    }),
                ))?;
                deleted_count += 1;
            }

            let batch_status = if skipped_modified_count > 0 {
                crate::queue::state_machine::STATUS_PARTIALLY_UNDONE
            } else {
                crate::queue::state_machine::STATUS_UNDONE
            };
            repo.update_batch_status(&batch_id, batch_status)?;
            repo.insert_event(&DomainEvent::new(
                "batch",
                batch_id.clone(),
                "batch.undo_completed",
                serde_json::json!({
                    "status": batch_status,
                    "deleted": deleted_count,
                    "skipped_modified": skipped_modified_count,
                    "force": force
                }),
            ))?;

            println!(
                "Undo complete: {} deleted, {} skipped (modified).",
                deleted_count, skipped_modified_count
            );
            if skipped_modified_count > 0 {
                Ok(exit_codes::PARTIAL_SUCCESS)
            } else {
                Ok(exit_codes::SUCCESS)
            }
        }
    }
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

fn ensure_job_output_mode(json: bool, table: bool) -> Result<()> {
    if json && table {
        anyhow::bail!("--json and --table cannot be used together");
    }
    Ok(())
}

fn format_job_list_header() -> String {
    format!(
        "{:<36}  {:<18}  {:>5}  {:<12}  {}",
        "batch_id", "status", "items", "target", "created_at"
    )
}

fn format_job_list_row(row: &BatchJob) -> String {
    format!(
        "{:<36}  {:<18}  {:>5}  {:<12}  {}",
        truncate_cell(&row.batch_id, 36),
        truncate_cell(&row.status, 18),
        row.source_count,
        truncate_cell(&row.target_id, 12),
        row.created_at
    )
}

fn format_job_show_header() -> String {
    format!(
        "{:<36}  {:<22}  {:<12}  {:<32}  {:<32}  {}",
        "item_id", "status", "target", "source", "stored", "error"
    )
}

fn format_job_show_row(item: &ItemJob) -> String {
    let error = match (&item.error_code, &item.error_message) {
        (Some(code), Some(message)) => format!("{code}: {message}"),
        (Some(code), None) => code.clone(),
        _ => String::new(),
    };
    format!(
        "{:<36}  {:<22}  {:<12}  {:<32}  {:<32}  {}",
        truncate_cell(&item.item_id, 36),
        truncate_cell(&item.status, 22),
        truncate_cell(&item.target_id, 12),
        truncate_cell(&item.source_path, 32),
        truncate_cell(item.stored_path.as_deref().unwrap_or("-"), 32),
        truncate_cell(&error, 40)
    )
}

fn truncate_cell(value: &str, width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let prefix = value.chars().take(width - 3).collect::<String>();
    format!("{prefix}...")
}

#[derive(Debug, Serialize)]
struct JobListRow {
    batch_id: String,
    status: String,
    source_count: i64,
    target_id: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct JobShowRow {
    batch_id: String,
    status: String,
    source: String,
    source_count: i64,
    created_at: String,
    updated_at: String,
    items: Vec<JobShowItemRow>,
}

#[derive(Debug, Serialize)]
struct JobShowItemRow {
    item_id: String,
    status: String,
    target_id: String,
    source_path: String,
    stored_path: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
}

fn parse_batch_status_filter(status: Option<String>) -> Result<Option<String>> {
    let Some(status) = status else {
        return Ok(None);
    };
    let status = status.trim().to_ascii_lowercase();
    let valid = [
        crate::queue::state_machine::STATUS_QUEUED,
        crate::queue::state_machine::STATUS_RUNNING,
        crate::queue::state_machine::STATUS_SUCCESS,
        crate::queue::state_machine::STATUS_FAILED,
        crate::queue::state_machine::STATUS_DUPLICATE,
        crate::queue::state_machine::STATUS_UNDONE,
        crate::queue::state_machine::STATUS_PARTIALLY_UNDONE,
    ];
    if valid.contains(&status.as_str()) {
        return Ok(Some(status));
    }

    anyhow::bail!("unsupported status filter: {status}");
}

pub fn handle_doctor(app: &App, fix: bool, migrate: bool) -> Result<i32> {
    let mut failed = false;
    println!("Config dir: {}", app.config.app_data_dir.display());
    println!("Database: {}", app.db_path.display());

    let config_path = app.config.config_path();
    if config_path.exists() {
        print_doctor_ok("Config file", &format!("{}", config_path.display()));
    } else {
        failed = true;
        print_doctor_fail(
            "Config file",
            &format!("missing at {}", config_path.display()),
            "Run: kbintake doctor --fix",
        );
    }

    match app.open_conn() {
        Ok(conn) => {
            if migrate {
                crate::db::apply_pending_migrations(&conn)?;
            }
            match crate::db::validate_schema(&conn) {
                Ok(()) => {
                    let version = crate::db::current_schema_version(&conn)?;
                    print_doctor_ok(
                        "Database schema",
                        &format!("Schema version: {} (up to date)", version),
                    );
                }
                Err(err) => {
                    failed = true;
                    print_doctor_fail(
                        "Database schema",
                        &err.to_string(),
                        "Check that the app data directory is writable; run: kbintake doctor --migrate",
                    );
                }
            }
        }
        Err(err) => {
            failed = true;
            print_doctor_fail(
                "Database schema",
                &err.to_string(),
                "Check that the app data directory is writable; run: kbintake doctor --migrate",
            );
        }
    }

    match app.config.default_target() {
        Ok(target) => {
            println!(
                "Default target: {} ({})",
                target.name,
                target.root_path.display()
            );
            match check_target_root(&target.root_path, fix) {
                Ok(()) => print_doctor_ok(
                    "Target directory",
                    &format!("{}", target.root_path.display()),
                ),
                Err(DoctorFailure { message, hint }) => {
                    failed = true;
                    print_doctor_fail("Target directory", &message, &hint);
                }
            }
        }
        Err(err) => {
            failed = true;
            print_doctor_fail(
                "Default target",
                &err.to_string(),
                "Run: kbintake config set-target <path>",
            );
        }
    }

    if crate::explorer::is_installed().unwrap_or(false) {
        print_doctor_ok("Explorer context menu", "registered");
    } else {
        print_doctor_warn(
            "Explorer context menu",
            "not registered",
            "Run: kbintake explorer install",
        );
    }

    for warning in app.config.routing_warnings() {
        print_doctor_warn(
            "Routing",
            &warning,
            "Run: kbintake targets add <name> <path> or update config.toml",
        );
    }

    if command_on_path("kbintake") {
        print_doctor_ok("PATH", "kbintake found");
    } else {
        print_doctor_warn(
            "PATH",
            "kbintake not found on PATH",
            "Add %LOCALAPPDATA%\\Programs\\kbintake to your PATH",
        );
    }

    if failed {
        Ok(exit_codes::GENERAL_ERROR)
    } else {
        Ok(exit_codes::SUCCESS)
    }
}

struct DoctorFailure {
    message: String,
    hint: String,
}

fn check_target_root(
    root_path: &std::path::Path,
    fix: bool,
) -> std::result::Result<(), DoctorFailure> {
    if root_path.exists() && !root_path.is_dir() {
        return Err(DoctorFailure {
            message: format!(
                "path exists but is not a directory: {}",
                root_path.display()
            ),
            hint: "Run: kbintake config set-target <existing-directory>".to_string(),
        });
    }
    if !root_path.exists() && !fix {
        return Err(DoctorFailure {
            message: format!("missing: {}", root_path.display()),
            hint: "Run: kbintake doctor --fix or kbintake config set-target <existing-directory>"
                .to_string(),
        });
    }
    config::validate_target_root(root_path).map_err(|err| DoctorFailure {
        message: err.to_string(),
        hint: "Check folder permissions or run: kbintake config set-target <writable-directory>"
            .to_string(),
    })
}

fn print_doctor_ok(check: &str, detail: &str) {
    println!("[OK] {check}: {detail}");
}

fn print_doctor_warn(check: &str, detail: &str, hint: &str) {
    println!("[WARN] {check}: {detail}");
    println!("  Hint: {hint}");
}

fn print_doctor_fail(check: &str, detail: &str, hint: &str) {
    println!("[FAIL] {check}: {detail}");
    println!("  Hint: {hint}");
}

fn command_on_path(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|path| {
            let candidate = path.join(command);
            candidate.exists() || candidate.with_extension("exe").exists()
        })
    })
}

pub fn handle_config_show(app: &App) -> Result<()> {
    println!("{}", toml::to_string_pretty(&app.config)?);
    Ok(())
}

pub fn handle_config(app: &App, command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show => handle_config_show(app),
        ConfigCommands::Validate => {
            let validation = app.config.validate_semantics();
            for warning in &validation.warnings {
                println!("[WARN] {warning}");
            }
            if validation.is_valid() {
                println!("Config validation succeeded.");
                return Ok(());
            }
            for error in &validation.errors {
                println!("[ERROR] {error}");
            }
            anyhow::bail!(
                "config validation failed with {} error(s)",
                validation.errors.len()
            )
        }
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
        ExplorerCommands::ComFeasibility => {
            let report = crate::explorer::com_probe::probe()?;
            println!("Windows 11 Explorer COM feasibility probe");
            for line in report.lines() {
                println!("{line}");
            }
            Ok(())
        }
        ExplorerCommands::RunImport { .. } => {
            anyhow::bail!("explorer run-import is only intended for the hidden GUI launcher")
        }
    }
}

pub fn handle_explorer_run_import(app: &App, queue_only: bool, paths: Vec<PathBuf>) -> Result<i32> {
    let outcome = handle_import(app, None, paths)?;
    if queue_only {
        let toast = ToastContent {
            title: "KBIntake".to_string(),
            line1: queued_toast_line(&outcome),
            line2: Some(format!("Batch {}", outcome.batch_id)),
        };
        show_explorer_toast(&toast);
        return Ok(exit_codes::SUCCESS);
    }

    scheduler::drain_queue(app)?;
    let summary = summarize_batch(app, &outcome.batch_id)?;
    let toast = toast_for_batch(&summary, &outcome.routing_summary);
    show_explorer_toast(&toast);

    if summary.failed > 0 && summary.imported == 0 && summary.duplicates == 0 {
        Ok(exit_codes::GENERAL_ERROR)
    } else if summary.failed > 0 {
        Ok(exit_codes::PARTIAL_SUCCESS)
    } else {
        Ok(exit_codes::SUCCESS)
    }
}

pub fn handle_explorer_run_import_error(err: &anyhow::Error) {
    let toast = ToastContent {
        title: "KBIntake".to_string(),
        line1: "Import failed before processing finished.".to_string(),
        line2: Some(err.to_string()),
    };
    let icon_path = std::env::current_exe()
        .ok()
        .and_then(|exe_path| crate::explorer::discover_icon_next_to_exe(&exe_path));
    let _ = crate::notify::show_toast(&toast, icon_path.as_deref());
}

fn summarize_batch(app: &App, batch_id: &str) -> Result<ExplorerBatchSummary> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let batch = repo.get_batch(batch_id)?;
    let items = repo.list_items_by_batch(batch_id)?;
    let mut imported = 0usize;
    let mut duplicates = 0usize;
    let mut failed = 0usize;
    for item in items {
        match item.status.as_str() {
            crate::queue::state_machine::STATUS_SUCCESS => imported += 1,
            crate::queue::state_machine::STATUS_DUPLICATE => duplicates += 1,
            crate::queue::state_machine::STATUS_FAILED => failed += 1,
            _ => {}
        }
    }
    let target_name = if batch.target_id == "mixed" {
        "mixed".to_string()
    } else {
        app.config.target_any_by_id(&batch.target_id)?.name
    };
    Ok(ExplorerBatchSummary {
        batch_id: batch.batch_id,
        imported,
        duplicates,
        failed,
        target_name,
    })
}

fn queued_toast_line(outcome: &ImportOutcome) -> String {
    match &outcome.routing_summary {
        RoutingSummary::Single(rule) => format!(
            "Queued {} item(s) for {} using rule {}.",
            outcome.item_count, outcome.target_name, rule
        ),
        RoutingSummary::Multiple => format!(
            "Queued {} item(s) for {} using multiple rules.",
            outcome.item_count, outcome.target_name
        ),
        RoutingSummary::None => {
            format!(
                "Queued {} item(s) for {}.",
                outcome.item_count, outcome.target_name
            )
        }
    }
}

fn toast_for_batch(
    summary: &ExplorerBatchSummary,
    routing_summary: &RoutingSummary,
) -> ToastContent {
    if summary.failed == 0 {
        let detail = if summary.duplicates > 0 {
            format!("{} duplicate skipped.", summary.duplicates)
        } else {
            "No duplicates skipped.".to_string()
        };
        let line1 = match routing_summary {
            RoutingSummary::Single(rule) => format!(
                "Imported {} file(s) into {} using rule {}.",
                summary.imported, summary.target_name, rule
            ),
            RoutingSummary::Multiple => format!(
                "Imported {} file(s) into {} using multiple rules.",
                summary.imported, summary.target_name
            ),
            RoutingSummary::None => format!(
                "Imported {} file(s) into {}.",
                summary.imported, summary.target_name
            ),
        };
        ToastContent {
            title: "KBIntake".to_string(),
            line1,
            line2: Some(detail),
        }
    } else {
        let line1 = match routing_summary {
            RoutingSummary::Single(rule) => {
                format!(
                    "Import finished with {} failure(s) after rule {}.",
                    summary.failed, rule
                )
            }
            RoutingSummary::Multiple => {
                format!(
                    "Import finished with {} failure(s) after multiple rules.",
                    summary.failed
                )
            }
            RoutingSummary::None => {
                format!("Import finished with {} failure(s).", summary.failed)
            }
        };
        ToastContent {
            title: "KBIntake".to_string(),
            line1,
            line2: Some(format!("Run: kbintake jobs retry {}", summary.batch_id)),
        }
    }
}

fn show_explorer_toast(toast: &ToastContent) {
    let icon_path = std::env::current_exe()
        .ok()
        .and_then(|exe_path| crate::explorer::discover_icon_next_to_exe(&exe_path));
    let _ = crate::notify::show_toast(toast, icon_path.as_deref());
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
        VaultCommands::Stats { target, json } => handle_vault_stats(app, target, json),
    }
}

fn handle_vault_stats(app: &App, target_filter: Option<String>, json: bool) -> Result<()> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let mut rows = Vec::new();
    let targets = if let Some(target_filter) = target_filter {
        let target = app.config.target_any_by_id(&target_filter)?;
        vec![target.clone()]
    } else {
        app.config
            .targets
            .iter()
            .filter(|t| t.is_active())
            .cloned()
            .collect::<Vec<_>>()
    };
    for target in targets {
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
            is_default: app
                .config
                .targets
                .first()
                .is_some_and(|t| t.target_id == target.target_id),
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
        format_event_line, format_job_show_row, handle_config, handle_explorer, handle_import,
        handle_import_command, handle_targets, queued_toast_line, toast_for_batch, ConfigCommands,
        ExplorerBatchSummary, RoutingSummary, TargetCommands,
    };
    use crate::app::App;
    use crate::config::{AgentConfig, AppConfig, ImportConfig};
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
                    inject_frontmatter: true,
                },
                agent: AgentConfig {
                    poll_interval_secs: 5,
                },
                routing: Vec::new(),
                templates: Vec::new(),
                routing_rules: Vec::new(),
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
    fn import_uses_v2_routing_rule_target_without_explicit_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();
        app.config.routing_rules.push(crate::config::RoutingRuleV2 {
            extension: Some(crate::config::StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "pdf-template".to_string(),
            target: Some("archive".to_string()),
        });
        let source = temp.path().join("paper.pdf");
        fs::write(&source, "pdf").unwrap();

        let outcome = handle_import(&app, None, vec![source]).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.get_batch(&outcome.batch_id).unwrap();
        let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
        assert_eq!(outcome.target_name, "archive");
        assert_eq!(
            outcome.routing_summary,
            RoutingSummary::Single("pdf-template".to_string())
        );
        assert_eq!(batch.target_id, "archive");
        assert_eq!(items[0].target_id, "archive");
    }

    #[test]
    fn explicit_target_overrides_v2_routing_rule_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app(&temp);
        app.config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();
        app.config.routing_rules.push(crate::config::RoutingRuleV2 {
            extension: Some(crate::config::StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "pdf-template".to_string(),
            target: Some("archive".to_string()),
        });
        let source = temp.path().join("paper.pdf");
        fs::write(&source, "pdf").unwrap();

        let outcome = handle_import(&app, Some("default".to_string()), vec![source]).unwrap();

        let conn = app.open_conn().unwrap();
        let repo = Repository::new(&conn);
        let batch = repo.get_batch(&outcome.batch_id).unwrap();
        let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
        assert_eq!(outcome.target_name, "default");
        assert_eq!(outcome.routing_summary, RoutingSummary::None);
        assert_eq!(batch.target_id, "default");
        assert_eq!(items[0].target_id, "default");
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
    fn jobs_show_row_includes_target_and_failure_details() {
        let mut item = ItemJob::new(
            "batch".to_string(),
            "archive".to_string(),
            "source.md".into(),
        );
        item.status = "failed".to_string();
        item.error_code = Some("E_TEST".to_string());
        item.error_message = Some("test failure".to_string());

        let line = format_job_show_row(&item);

        assert!(line.contains("archive"));
        assert!(line.contains("E_TEST: test failure"));
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

    #[test]
    fn explorer_success_toast_mentions_import_and_duplicates() {
        let toast = toast_for_batch(
            &ExplorerBatchSummary {
                batch_id: "batch-1".to_string(),
                imported: 3,
                duplicates: 1,
                failed: 0,
                target_name: "notes".to_string(),
            },
            &RoutingSummary::Single("research-paper".to_string()),
        );

        assert_eq!(toast.title, "KBIntake");
        assert!(toast
            .line1
            .contains("Imported 3 file(s) into notes using rule research-paper."));
        assert_eq!(toast.line2.as_deref(), Some("1 duplicate skipped."));
    }

    #[test]
    fn explorer_failure_toast_includes_retry_hint() {
        let toast = toast_for_batch(
            &ExplorerBatchSummary {
                batch_id: "batch-9".to_string(),
                imported: 1,
                duplicates: 0,
                failed: 2,
                target_name: "notes".to_string(),
            },
            &RoutingSummary::Multiple,
        );

        assert_eq!(toast.title, "KBIntake");
        assert!(toast.line1.contains("2 failure"));
        assert!(toast.line1.contains("multiple rules"));
        assert_eq!(
            toast.line2.as_deref(),
            Some("Run: kbintake jobs retry batch-9")
        );
    }

    #[test]
    fn queued_toast_line_mentions_single_routing_rule() {
        let line = queued_toast_line(&super::ImportOutcome {
            batch_id: "batch-1".to_string(),
            item_count: 2,
            target_name: "notes".to_string(),
            routing_summary: RoutingSummary::Single("research-paper".to_string()),
        });

        assert!(line.contains("Queued 2 item(s) for notes using rule research-paper."));
    }

    #[test]
    fn explorer_com_feasibility_command_executes() {
        handle_explorer(super::ExplorerCommands::ComFeasibility).unwrap();
    }
}
