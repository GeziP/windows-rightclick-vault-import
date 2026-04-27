//! Watch Mode — file system watcher that queues new files into the import pipeline.
//!
//! Uses the `notify` crate for OS-level file events (ReadDirectoryChangesW on Windows)
//! with a manual debounce layer to avoid processing files still being written.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use tracing::{error, info, warn};

use crate::app::App;
use crate::config::WatchConfig;
use crate::domain::BatchJob;
use crate::domain::ItemJob;
use crate::i18n::tr;
use crate::notify::ToastContent;
use crate::queue::repository::Repository;

/// Maximum retries for files that are still locked by the writing process.
const MAX_LOCK_RETRIES: u32 = 3;
const LOCK_RETRY_DELAY_MS: u64 = 1000;

/// Tracks per-file events for debouncing.
struct FileDebounceState {
    /// Last time we saw any event for this file.
    last_event: Instant,
    /// The file path (canonical if possible).
    path: PathBuf,
}

struct DebounceTracker {
    files: HashMap<PathBuf, FileDebounceState>,
    debounce_duration: Duration,
}

impl DebounceTracker {
    fn new(debounce_secs: u64) -> Self {
        Self {
            files: HashMap::new(),
            debounce_duration: Duration::from_secs(debounce_secs),
        }
    }

    fn record_event(&mut self, path: PathBuf) {
        let now = Instant::now();
        self.files.entry(path.clone()).or_insert_with(|| FileDebounceState {
            last_event: now,
            path: path.clone(),
        });
        self.files.get_mut(&path).unwrap().last_event = now;
    }

    /// Returns files that have been stable (no events) for the debounce duration.
    fn drain_stable(&mut self) -> Vec<PathBuf> {
        let cutoff = Instant::now()
            .checked_sub(self.debounce_duration)
            .unwrap_or(Instant::now());

        let mut stable = Vec::new();
        self.files.retain(|_path, state| {
            if state.last_event <= cutoff {
                stable.push(state.path.clone());
                false
            } else {
                true
            }
        });
        stable
    }
}

/// Start watching configured directories and queueing new files for import.
///
/// This is a long-running operation. Pass `watch_paths` to watch specific
/// directories (CLI override), or `None` to use the `[[watch]]` config entries.
pub fn run_watcher(app: &App, watch_paths: Option<Vec<PathBuf>>) -> Result<()> {
    let lang = app.config.language();
    let lock_path = app.config.app_data_dir.join("kbintake-watch.pid");

    // Check for duplicate watcher (PID lock file).
    if let Err(_e) = acquire_watcher_lock(&lock_path) {
        anyhow::bail!("{}", tr("watcher.duplicate", lang));
    }

    let watch_configs = resolve_watch_configs(app, watch_paths)?;

    if watch_configs.is_empty() {
        anyhow::bail!(
            "no watch paths configured. Add [[watch]] sections to config.toml \
             or use `kbintake watch --path <dir>`"
        );
    }

    // Release lock on exit (best-effort).
    let _lock_guard = WatcherLockGuard::new(lock_path);

    let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();

    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(tx).context("failed to create file watcher")?;

    for config in &watch_configs {
        info!(
            "watching directory: {} (target: {:?}, extensions: {:?})",
            config.path.display(),
            config.target,
            config.extensions
        );
        let path = config.path.clone();
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch directory: {}", path.display()))?;
    }

    info!(
        "watcher started with {} path(s), debounce {}s",
        watch_configs.len(),
        watch_configs.first().map(|c| c.debounce_secs).unwrap_or(2)
    );

    // Collect all watched configs keyed by parent directory prefix for matching.
    let mut debounce_map: HashMap<PathBuf, DebounceTracker> = HashMap::new();
    for config in &watch_configs {
        debounce_map.insert(
            config.path.clone(),
            DebounceTracker::new(config.debounce_secs),
        );
    }

    let mut stable_queue_count = 0u64;

    // Main event loop — process events and check for stable files every 200ms.
    loop {
        // Non-blocking collect of recent events.
        while let Ok(result) = rx.try_recv() {
            match result {
                Ok(event) => handle_event(&mut debounce_map, &event),
                Err(e) => warn!("watcher error: {:#}", e),
            }
        }

        // Check for stable files and queue them.
        for (_watch_root, tracker) in debounce_map.iter_mut() {
            let stable_files = tracker.drain_stable();
            for file_path in stable_files {
                let config = watch_configs
                    .iter()
                    .find(|c| file_path.starts_with(&c.path))
                    .unwrap_or_else(|| {
                        watch_configs.first().expect("non-empty watch_configs")
                    });

                match process_stable_file(app, config, &file_path) {
                    Ok(true) => {
                        stable_queue_count += 1;
                        info!(
                            "queued stable file: {} (total queued: {})",
                            file_path.display(),
                            stable_queue_count
                        );
                    }
                    Ok(false) => {
                        // Skipped (duplicate, filtered out, etc.)
                    }
                    Err(e) => {
                        error!(
                            "failed to process stable file {}: {:#}",
                            file_path.display(),
                            e
                        );
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(200));
    }
}

fn handle_event(
    debounce_map: &mut HashMap<PathBuf, DebounceTracker>,
    event: &Event,
) {
    // Only care about file creation and modification events.
    use notify::EventKind;
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {}
        _ => return,
    }

    for path in &event.paths {
        if path.is_file() {
            // Find the matching watch root and record the event.
            for (root, tracker) in debounce_map.iter_mut() {
                if path.starts_with(root) {
                    tracker.record_event(path.clone());
                }
            }
        }
    }
}

fn process_stable_file(
    app: &App,
    config: &WatchConfig,
    file_path: &Path,
) -> Result<bool> {
    // Extension filter check.
    if let Some(extensions) = &config.extensions {
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            let ext_with_dot = format!(".{}", ext);
            if !extensions.matches_case_insensitive(&ext_with_dot) {
                return Ok(false); // Skip — extension not in watch list
            }
        } else {
            return Ok(false); // Skip — no extension
        }
    }

    // Check if file is still accessible and not locked.
    let metadata = match retry_locked(file_path, || Ok(std::fs::metadata(file_path)?)) {
        Ok(m) => m,
        Err(_) => {
            warn!("file still locked after retries, skipping: {}", file_path.display());
            return Ok(false);
        }
    };
    let size = metadata.len();

    // Resolve target and template using the same engine as CLI import.
    let intent = app.config.resolve_import_intent(
        file_path,
        size,
        config.target.clone(),
        config.template.clone(),
    )?;

    // Queue the file into the import pipeline.
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);

    // Create batch with source="watcher".
    let batch = BatchJob::new("watcher", &intent.target.target_id, 1);
    repo.insert_batch(&batch)?;
    repo.insert_event(&crate::domain::DomainEvent::new(
        "batch",
        batch.batch_id.clone(),
        "batch.queued",
        serde_json::json!({
            "source": batch.source,
            "target_id": batch.target_id,
            "source_count": batch.source_count,
            "watched_path": config.path.display().to_string(),
        }),
    ))?;

    // Create item.
    let item = ItemJob::new(batch.batch_id.clone(), intent.target.target_id, file_path.to_path_buf());
    repo.insert_item(&item)?;
    repo.insert_event(&crate::domain::DomainEvent::new(
        "item",
        item.item_id.clone(),
        "item.queued",
        serde_json::json!({
            "batch_id": item.batch_id,
            "target_id": item.target_id,
            "source_path": item.source_path,
        }),
    ))?;

    // Process the queued item immediately and show toast notification.
    let batch_id = batch.batch_id.clone();
    let source_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if crate::agent::scheduler::process_next_item(app).is_ok() {
        let toast = build_watch_toast(app, &batch_id, &source_name);
        let _ = crate::notify::show_toast(&toast, None);
    }

    Ok(true)
}

/// Build a toast notification summarizing the result of a watch import.
fn build_watch_toast(app: &App, batch_id: &str, source_name: &str) -> ToastContent {
    let lang = app.config.language();
    let conn = app.open_conn().unwrap_or_else(|_| panic!("db open"));
    let repo = Repository::new(&conn);
    let status = repo.get_batch(batch_id).map(|b| b.status).ok();

    match status.as_deref() {
        Some("success") => ToastContent {
            title: tr("toast.watch_import_ok_title", lang),
            line1: tr("toast.watch_import_ok", lang).replace("{file}", source_name),
            line2: None,
        },
        Some("duplicate") | Some("failed") => ToastContent {
            title: tr("toast.watch_import_warn_title", lang),
            line1: tr("toast.watch_import_warn", lang).replace("{file}", source_name),
            line2: status.map(|s| format!("Status: {s}")),
        },
        _ => ToastContent {
            title: tr("toast.watch_import_queued_title", lang),
            line1: tr("toast.watch_import_queued", lang).replace("{file}", source_name),
            line2: None,
        },
    }
}

/// Retry an operation that may fail because the file is still locked.
fn retry_locked<T, F>(path: &Path, mut f: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut last_err = None;
    for attempt in 0..MAX_LOCK_RETRIES {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) => {
                warn!(
                    "file locked on attempt {}/{}, retrying in {}ms: {}",
                    attempt + 1,
                    MAX_LOCK_RETRIES,
                    LOCK_RETRY_DELAY_MS,
                    path.display()
                );
                last_err = Some(e);
                std::thread::sleep(Duration::from_millis(LOCK_RETRY_DELAY_MS));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("file locked after {} retries", MAX_LOCK_RETRIES)))
}

fn resolve_watch_configs(
    app: &App,
    cli_paths: Option<Vec<PathBuf>>,
) -> Result<Vec<WatchConfig>> {
    if let Some(paths) = cli_paths {
        return paths
            .into_iter()
            .map(|path| {
                anyhow::ensure!(
                    path.is_dir(),
                    "watch path is not a directory: {}",
                    path.display()
                );
                Ok(WatchConfig {
                    path: path.clone(),
                    target: None,
                    debounce_secs: 2,
                    extensions: None,
                    template: None,
                })
            })
            .collect();
    }

    if app.config.watch.is_empty() {
        return Ok(vec![]);
    }

    // Validate that all configured watch paths exist.
    for config in &app.config.watch {
        anyhow::ensure!(
            config.path.is_dir(),
            "watch path does not exist: {}",
            config.path.display()
        );
    }

    Ok(app.config.watch.clone())
}

/// Try to acquire a PID-based lock. Returns `Err` if another watcher is running.
fn acquire_watcher_lock(lock_path: &Path) -> Result<()> {
    if lock_path.exists() {
        let content = fs::read_to_string(lock_path).unwrap_or_default();
        if let Ok(pid) = content.trim().parse::<u32>() {
            // Check if the process is still alive.
            if is_process_alive(pid) {
                return Err(anyhow::anyhow!("duplicate watcher PID {pid}"));
            }
            // Stale lock — remove it.
            let _ = fs::remove_file(lock_path);
        }
    }
    Ok(())
}

/// Check if a process with the given PID is alive.
#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
        let Ok(handle) = handle else {
            return false; // Process doesn't exist or access denied
        };
        let mut exit_code: u32 = 0;
        if GetExitCodeProcess(handle, &mut exit_code).is_ok() {
            // STILL_ACTIVE = 259
            exit_code == 259
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
fn is_process_alive(pid: u32) -> bool {
    if let Ok(status) = std::fs::read(format!("/proc/{pid}/status")) {
        !status.is_empty()
    } else {
        false
    }
}

/// RAII guard that removes the PID lock file on drop.
struct WatcherLockGuard {
    path: PathBuf,
}

impl WatcherLockGuard {
    fn new(path: PathBuf) -> Self {
        // Write the current PID to the lock file.
        let pid = std::process::id();
        let _ = fs::write(&path, pid.to_string());
        Self { path }
    }
}

impl Drop for WatcherLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
