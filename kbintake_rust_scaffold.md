# KBIntake Rust 项目骨架（v0.1）

下面这套骨架按我们前面定下的方案组织：**单 exe、CLI + agent、单 SQLite、本地目录后端、Windows 右键接入**。

---

## 1. 目录结构

```text
kbintake/
├─ Cargo.toml
├─ src/
│  ├─ main.rs
│  ├─ app.rs
│  ├─ cli/
│  │  └─ mod.rs
│  ├─ config/
│  │  └─ mod.rs
│  ├─ db/
│  │  ├─ mod.rs
│  │  └─ schema.rs
│  ├─ domain/
│  │  ├─ mod.rs
│  │  ├─ batch.rs
│  │  ├─ item.rs
│  │  ├─ manifest.rs
│  │  ├─ target.rs
│  │  └─ event.rs
│  ├─ queue/
│  │  ├─ mod.rs
│  │  ├─ repository.rs
│  │  └─ state_machine.rs
│  ├─ processor/
│  │  ├─ mod.rs
│  │  ├─ scanner.rs
│  │  ├─ validator.rs
│  │  ├─ hasher.rs
│  │  ├─ deduper.rs
│  │  └─ copier.rs
│  ├─ adapter/
│  │  ├─ mod.rs
│  │  └─ local_folder.rs
│  ├─ agent/
│  │  ├─ mod.rs
│  │  ├─ scheduler.rs
│  │  └─ worker.rs
│  └─ logging/
│     └─ mod.rs
└─ scripts/
   ├─ register_file_context_menu.reg
   └─ register_dir_context_menu.reg
```

---

## 2. Cargo.toml

```toml
[package]
name = "kbintake"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
dirs = "5"
rusqlite = { version = "0.31", features = ["bundled", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
thiserror = "1"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
uuid = { version = "1", features = ["v4", "serde"] }
walkdir = "2"
```

---

## 3. main.rs

```rust
mod app;
mod cli;
mod config;
mod db;
mod domain;
mod queue;
mod processor;
mod adapter;
mod agent;
mod logging;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    logging::init_logging()?;

    let cli = Cli::parse();
    let app = app::App::bootstrap()?;

    match cli.command {
        Commands::Agent => agent::run_agent(&app)?,
        Commands::Import { paths } => cli::handle_import(&app, paths)?,
        Commands::Jobs { command } => cli::handle_jobs(&app, command)?,
        Commands::Doctor => cli::handle_doctor(&app)?,
        Commands::ConfigShow => cli::handle_config_show(&app)?,
    }

    Ok(())
}
```

---

## 4. app.rs

```rust
use anyhow::Result;
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::db;

pub struct App {
    pub config: AppConfig,
    pub db_path: std::path::PathBuf,
}

impl App {
    pub fn bootstrap() -> Result<Self> {
        let config = AppConfig::load_or_init()?;
        let db_path = config.app_data_dir.join("data").join("kbintake.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        db::init_schema(&conn)?;
        drop(conn);

        Ok(Self { config, db_path })
    }

    pub fn open_conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }
}
```

---

## 5. cli/mod.rs

```rust
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use crate::app::App;
use crate::domain::BatchJob;
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
    Import { paths: Vec<PathBuf> },
    Jobs {
        #[command(subcommand)]
        command: JobCommands,
    },
    Doctor,
    ConfigShow,
}

#[derive(Subcommand, Debug)]
pub enum JobCommands {
    List,
    Show { batch_id: String },
}

pub fn handle_import(app: &App, paths: Vec<PathBuf>) -> Result<()> {
    if paths.is_empty() {
        anyhow::bail!("no input paths provided");
    }

    let target = app.config.default_target()?;
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);

    let batch = BatchJob::new("cli", &target.target_id, 0);
    repo.insert_batch(&batch)?;

    let mut count = 0usize;
    for path in paths {
        let discovered = scanner::expand_input_path(&path)
            .with_context(|| format!("failed to scan path {}", path.display()))?;
        for file in discovered {
            let item = crate::domain::ItemJob::new(batch.batch_id.clone(), target.target_id.clone(), file);
            repo.insert_item(&item)?;
            count += 1;
        }
    }

    repo.update_batch_source_count(&batch.batch_id, count as i64)?;
    info!(batch_id = %batch.batch_id, items = count, "batch queued");
    println!("Queued batch: {}", batch.batch_id);
    println!("Items queued: {}", count);
    println!("Target: {}", target.name);
    Ok(())
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
                println!(
                    "- {} [{}] {} -> {}",
                    item.item_id,
                    item.status,
                    item.source_path,
                    item.stored_path.unwrap_or_else(|| "-".to_string())
                );
            }
        }
    }
    Ok(())
}

pub fn handle_doctor(app: &App) -> Result<()> {
    println!("DB: {}", app.db_path.display());
    println!("App data dir: {}", app.config.app_data_dir.display());
    let target = app.config.default_target()?;
    println!("Default target: {} -> {}", target.name, target.root_path.display());
    Ok(())
}

pub fn handle_config_show(app: &App) -> Result<()> {
    println!("{}", toml::to_string_pretty(&app.config)?);
    Ok(())
}
```

---

## 6. config/mod.rs

```rust
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_data_dir: PathBuf,
    pub agent: AgentConfig,
    pub import: ImportConfig,
    pub targets: Vec<TargetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    pub mode: String,
    pub max_file_size_mb: u64,
    pub ignore_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub target_id: String,
    pub name: String,
    pub root_path: PathBuf,
    pub adapter_type: String,
    pub is_default: bool,
}

impl AppConfig {
    pub fn load_or_init() -> Result<Self> {
        let base = dirs::data_local_dir()
            .context("failed to determine LOCALAPPDATA")?
            .join("KBIntake");
        let config_dir = base.join("config");
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            std::fs::create_dir_all(&config_dir)?;
            let default = Self::default_with_base(base.clone());
            std::fs::write(&config_path, toml::to_string_pretty(&default)?)?;
            return Ok(default);
        }

        let text = std::fs::read_to_string(&config_path)?;
        let mut cfg: Self = toml::from_str(&text)?;
        cfg.app_data_dir = base;
        Ok(cfg)
    }

    fn default_with_base(base: PathBuf) -> Self {
        Self {
            app_data_dir: base,
            agent: AgentConfig { max_concurrency: 1 },
            import: ImportConfig {
                mode: "copy".to_string(),
                max_file_size_mb: 512,
                ignore_hidden: true,
            },
            targets: vec![TargetConfig {
                target_id: "default".to_string(),
                name: "Default Knowledge Base".to_string(),
                root_path: PathBuf::from("D:/KnowledgeBase"),
                adapter_type: "local_folder".to_string(),
                is_default: true,
            }],
        }
    }

    pub fn default_target(&self) -> Result<&TargetConfig> {
        self.targets
            .iter()
            .find(|t| t.is_default)
            .context("no default target configured")
    }
}
```

---

## 7. db/mod.rs

```rust
use anyhow::Result;
use rusqlite::Connection;

pub mod schema;

pub fn init_schema(conn: &Connection) -> Result<()> {
    schema::init(conn)?;
    Ok(())
}
```

## 8. db/schema.rs

```rust
use anyhow::Result;
use rusqlite::Connection;

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS batch_jobs (
          batch_id TEXT PRIMARY KEY,
          trigger_source TEXT NOT NULL,
          target_id TEXT NOT NULL,
          source_count INTEGER NOT NULL,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS item_jobs (
          item_id TEXT PRIMARY KEY,
          batch_id TEXT NOT NULL,
          target_id TEXT NOT NULL,
          source_path TEXT NOT NULL,
          source_kind TEXT NOT NULL,
          source_name TEXT NOT NULL,
          file_ext TEXT,
          file_size INTEGER,
          status TEXT NOT NULL,
          step TEXT,
          stored_path TEXT,
          hash_sha256 TEXT,
          duplicate_of_record_id TEXT,
          error_code TEXT,
          error_message TEXT,
          retry_count INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS manifest_records (
          record_id TEXT PRIMARY KEY,
          item_id TEXT NOT NULL,
          target_id TEXT NOT NULL,
          source_path TEXT NOT NULL,
          stored_path TEXT NOT NULL,
          source_name TEXT NOT NULL,
          file_ext TEXT,
          file_size INTEGER,
          hash_sha256 TEXT NOT NULL,
          imported_at TEXT NOT NULL,
          import_mode TEXT NOT NULL,
          metadata_json TEXT
        );

        CREATE TABLE IF NOT EXISTS job_events (
          event_id TEXT PRIMARY KEY,
          batch_id TEXT,
          item_id TEXT,
          level TEXT NOT NULL,
          event_type TEXT NOT NULL,
          message TEXT NOT NULL,
          payload_json TEXT,
          created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_item_jobs_status ON item_jobs(status);
        CREATE INDEX IF NOT EXISTS idx_item_jobs_batch_id ON item_jobs(batch_id);
        CREATE INDEX IF NOT EXISTS idx_manifest_hash_target ON manifest_records(target_id, hash_sha256);
        "#,
    )?;
    Ok(())
}
```

---

## 9. domain/mod.rs

```rust
pub mod batch;
pub mod item;
pub mod manifest;
pub mod target;
pub mod event;

pub use batch::BatchJob;
pub use item::{ItemJob, JobStatus};
pub use manifest::ManifestRecord;
pub use target::Target;
pub use event::JobEvent;
```

## 10. domain/batch.rs

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BatchJob {
    pub batch_id: String,
    pub trigger_source: String,
    pub target_id: String,
    pub source_count: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BatchJob {
    pub fn new(trigger_source: &str, target_id: &str, source_count: i64) -> Self {
        let now = Utc::now();
        Self {
            batch_id: Uuid::new_v4().to_string(),
            trigger_source: trigger_source.to_string(),
            target_id: target_id.to_string(),
            source_count,
            status: "queued".to_string(),
            created_at: now,
            updated_at: now,
        }
    }
}
```

## 11. domain/item.rs

```rust
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum JobStatus {
    Queued,
    Running,
    Success,
    Failed,
    DuplicateSkipped,
    Canceled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Success => "success",
            JobStatus::Failed => "failed",
            JobStatus::DuplicateSkipped => "duplicate_skipped",
            JobStatus::Canceled => "canceled",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub struct ItemJob {
    pub item_id: String,
    pub batch_id: String,
    pub target_id: String,
    pub source_path: String,
    pub source_kind: String,
    pub source_name: String,
    pub file_ext: Option<String>,
    pub file_size: Option<i64>,
    pub status: String,
    pub step: Option<String>,
    pub stored_path: Option<String>,
    pub hash_sha256: Option<String>,
    pub duplicate_of_record_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ItemJob {
    pub fn new(batch_id: String, target_id: String, source: PathBuf) -> Self {
        let now = Utc::now();
        let source_name = source
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| source.display().to_string());
        let file_ext = source.extension().map(|s| s.to_string_lossy().to_string());
        Self {
            item_id: Uuid::new_v4().to_string(),
            batch_id,
            target_id,
            source_path: source.to_string_lossy().to_string(),
            source_kind: "file".to_string(),
            source_name,
            file_ext,
            file_size: None,
            status: JobStatus::Queued.to_string(),
            step: None,
            stored_path: None,
            hash_sha256: None,
            duplicate_of_record_id: None,
            error_code: None,
            error_message: None,
            retry_count: 0,
            created_at: now,
            updated_at: now,
        }
    }
}
```

## 12. domain/manifest.rs

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ManifestRecord {
    pub record_id: String,
    pub item_id: String,
    pub target_id: String,
    pub source_path: String,
    pub stored_path: String,
    pub source_name: String,
    pub file_ext: Option<String>,
    pub file_size: Option<i64>,
    pub hash_sha256: String,
    pub imported_at: DateTime<Utc>,
    pub import_mode: String,
    pub metadata_json: Option<String>,
}

impl ManifestRecord {
    pub fn new(
        item_id: String,
        target_id: String,
        source_path: String,
        stored_path: String,
        source_name: String,
        file_ext: Option<String>,
        file_size: Option<i64>,
        hash_sha256: String,
    ) -> Self {
        Self {
            record_id: Uuid::new_v4().to_string(),
            item_id,
            target_id,
            source_path,
            stored_path,
            source_name,
            file_ext,
            file_size,
            hash_sha256,
            imported_at: Utc::now(),
            import_mode: "copy".to_string(),
            metadata_json: None,
        }
    }
}
```

## 13. domain/target.rs

```rust
#[derive(Debug, Clone)]
pub struct Target {
    pub target_id: String,
    pub name: String,
    pub root_path: String,
    pub adapter_type: String,
}
```

## 14. domain/event.rs

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JobEvent {
    pub event_id: String,
    pub batch_id: Option<String>,
    pub item_id: Option<String>,
    pub level: String,
    pub event_type: String,
    pub message: String,
    pub payload_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl JobEvent {
    pub fn new(level: &str, event_type: &str, message: &str) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            batch_id: None,
            item_id: None,
            level: level.to_string(),
            event_type: event_type.to_string(),
            message: message.to_string(),
            payload_json: None,
            created_at: Utc::now(),
        }
    }
}
```

---

## 15. queue/repository.rs

```rust
use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::domain::{BatchJob, ItemJob, ManifestRecord};

pub struct Repository<'a> {
    conn: &'a Connection,
}

impl<'a> Repository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert_batch(&self, batch: &BatchJob) -> Result<()> {
        self.conn.execute(
            "INSERT INTO batch_jobs (batch_id, trigger_source, target_id, source_count, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                batch.batch_id,
                batch.trigger_source,
                batch.target_id,
                batch.source_count,
                batch.status,
                batch.created_at.to_rfc3339(),
                batch.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn update_batch_source_count(&self, batch_id: &str, count: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE batch_jobs SET source_count = ?2, updated_at = datetime('now') WHERE batch_id = ?1",
            params![batch_id, count],
        )?;
        Ok(())
    }

    pub fn insert_item(&self, item: &ItemJob) -> Result<()> {
        self.conn.execute(
            "INSERT INTO item_jobs (
                item_id, batch_id, target_id, source_path, source_kind, source_name,
                file_ext, file_size, status, step, stored_path, hash_sha256,
                duplicate_of_record_id, error_code, error_message, retry_count,
                created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16,
                ?17, ?18
             )",
            params![
                item.item_id,
                item.batch_id,
                item.target_id,
                item.source_path,
                item.source_kind,
                item.source_name,
                item.file_ext,
                item.file_size,
                item.status,
                item.step,
                item.stored_path,
                item.hash_sha256,
                item.duplicate_of_record_id,
                item.error_code,
                item.error_message,
                item.retry_count,
                item.created_at.to_rfc3339(),
                item.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_batches(&self, limit: i64) -> Result<Vec<BatchJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT batch_id, trigger_source, target_id, source_count, status, created_at, updated_at
             FROM batch_jobs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(BatchJob {
                batch_id: row.get(0)?,
                trigger_source: row.get(1)?,
                target_id: row.get(2)?,
                source_count: row.get(3)?,
                status: row.get(4)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .to_utc(),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .unwrap()
                    .to_utc(),
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_batch(&self, batch_id: &str) -> Result<BatchJob> {
        let mut stmt = self.conn.prepare(
            "SELECT batch_id, trigger_source, target_id, source_count, status, created_at, updated_at
             FROM batch_jobs WHERE batch_id = ?1",
        )?;
        stmt.query_row([batch_id], |row| {
            Ok(BatchJob {
                batch_id: row.get(0)?,
                trigger_source: row.get(1)?,
                target_id: row.get(2)?,
                source_count: row.get(3)?,
                status: row.get(4)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .to_utc(),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .unwrap()
                    .to_utc(),
            })
        }).context("batch not found")
    }

    pub fn list_items_by_batch(&self, batch_id: &str) -> Result<Vec<ItemJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, batch_id, target_id, source_path, source_kind, source_name,
                    file_ext, file_size, status, step, stored_path, hash_sha256,
                    duplicate_of_record_id, error_code, error_message, retry_count,
                    created_at, updated_at
             FROM item_jobs WHERE batch_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([batch_id], |row| {
            Ok(ItemJob {
                item_id: row.get(0)?,
                batch_id: row.get(1)?,
                target_id: row.get(2)?,
                source_path: row.get(3)?,
                source_kind: row.get(4)?,
                source_name: row.get(5)?,
                file_ext: row.get(6)?,
                file_size: row.get(7)?,
                status: row.get(8)?,
                step: row.get(9)?,
                stored_path: row.get(10)?,
                hash_sha256: row.get(11)?,
                duplicate_of_record_id: row.get(12)?,
                error_code: row.get(13)?,
                error_message: row.get(14)?,
                retry_count: row.get(15)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .unwrap()
                    .to_utc(),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(17)?)
                    .unwrap()
                    .to_utc(),
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn next_queued_item(&self) -> Result<Option<ItemJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, batch_id, target_id, source_path, source_kind, source_name,
                    file_ext, file_size, status, step, stored_path, hash_sha256,
                    duplicate_of_record_id, error_code, error_message, retry_count,
                    created_at, updated_at
             FROM item_jobs WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(ItemJob {
                item_id: row.get(0)?,
                batch_id: row.get(1)?,
                target_id: row.get(2)?,
                source_path: row.get(3)?,
                source_kind: row.get(4)?,
                source_name: row.get(5)?,
                file_ext: row.get(6)?,
                file_size: row.get(7)?,
                status: row.get(8)?,
                step: row.get(9)?,
                stored_path: row.get(10)?,
                hash_sha256: row.get(11)?,
                duplicate_of_record_id: row.get(12)?,
                error_code: row.get(13)?,
                error_message: row.get(14)?,
                retry_count: row.get(15)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .unwrap()
                    .to_utc(),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(17)?)
                    .unwrap()
                    .to_utc(),
            }));
        }
        Ok(None)
    }

    pub fn update_item_running(&self, item_id: &str, step: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE item_jobs SET status='running', step=?2, updated_at=datetime('now') WHERE item_id=?1",
            params![item_id, step],
        )?;
        Ok(())
    }

    pub fn update_item_hash(&self, item_id: &str, hash: &str, size: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE item_jobs SET hash_sha256=?2, file_size=?3, updated_at=datetime('now') WHERE item_id=?1",
            params![item_id, hash, size],
        )?;
        Ok(())
    }

    pub fn mark_item_duplicate(&self, item_id: &str, record_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE item_jobs SET status='duplicate_skipped', duplicate_of_record_id=?2, updated_at=datetime('now') WHERE item_id=?1",
            params![item_id, record_id],
        )?;
        Ok(())
    }

    pub fn mark_item_success(&self, item_id: &str, stored_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE item_jobs SET status='success', stored_path=?2, updated_at=datetime('now') WHERE item_id=?1",
            params![item_id, stored_path],
        )?;
        Ok(())
    }

    pub fn mark_item_failed(&self, item_id: &str, error_code: &str, error_message: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE item_jobs SET status='failed', error_code=?2, error_message=?3, updated_at=datetime('now') WHERE item_id=?1",
            params![item_id, error_code, error_message],
        )?;
        Ok(())
    }

    pub fn insert_manifest(&self, record: &ManifestRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO manifest_records (
                record_id, item_id, target_id, source_path, stored_path, source_name,
                file_ext, file_size, hash_sha256, imported_at, import_mode, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.record_id,
                record.item_id,
                record.target_id,
                record.source_path,
                record.stored_path,
                record.source_name,
                record.file_ext,
                record.file_size,
                record.hash_sha256,
                record.imported_at.to_rfc3339(),
                record.import_mode,
                record.metadata_json,
            ],
        )?;
        Ok(())
    }

    pub fn find_manifest_by_hash(&self, target_id: &str, hash: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT record_id FROM manifest_records WHERE target_id=?1 AND hash_sha256=?2 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![target_id, hash])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
}
```

## 16. queue/state_machine.rs

```rust
use anyhow::{bail, Result};

pub fn ensure_item_transition(from: &str, to: &str) -> Result<()> {
    let ok = matches!(
        (from, to),
        ("queued", "running")
            | ("running", "success")
            | ("running", "failed")
            | ("running", "duplicate_skipped")
            | ("queued", "canceled")
    );

    if ok {
        Ok(())
    } else {
        bail!("invalid state transition: {} -> {}", from, to)
    }
}
```

---

## 17. processor/scanner.rs

```rust
use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

pub fn expand_input_path(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        anyhow::bail!("source path not found: {}", path.display());
    }

    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(path) {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
            if name == "Thumbs.db" || name == "desktop.ini" || name.starts_with("~$") {
                continue;
            }
            files.push(p.to_path_buf());
        }
    }

    files.sort();
    Ok(files)
}
```

## 18. processor/validator.rs

```rust
use std::path::Path;

use anyhow::Result;

pub fn validate_file(path: &Path, max_size_mb: u64) -> Result<u64> {
    if !path.exists() {
        anyhow::bail!("source not found");
    }
    if !path.is_file() {
        anyhow::bail!("source is not a file");
    }
    let meta = std::fs::metadata(path)?;
    let size = meta.len();
    if size > max_size_mb * 1024 * 1024 {
        anyhow::bail!("source exceeds max file size");
    }
    Ok(size)
}
```

## 19. processor/hasher.rs

```rust
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

pub fn sha256_file(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
```

## 20. processor/deduper.rs

```rust
use anyhow::Result;

use crate::queue::repository::Repository;

pub fn find_duplicate_record(repo: &Repository<'_>, target_id: &str, hash: &str) -> Result<Option<String>> {
    repo.find_manifest_by_hash(target_id, hash)
}
```

## 21. processor/copier.rs

```rust
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{Datelike, Utc};

pub fn build_dest_path(target_root: &Path, source_name: &str) -> Result<PathBuf> {
    let now = Utc::now();
    let dir = target_root
        .join("raw")
        .join("sources")
        .join(now.year().to_string())
        .join(format!("{:02}", now.month()));
    std::fs::create_dir_all(&dir)?;

    let mut path = dir.join(source_name);
    if !path.exists() {
        return Ok(path);
    }

    let original = Path::new(source_name);
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = original.extension().map(|e| e.to_string_lossy().to_string());

    for idx in 2..10000 {
        let candidate = match &ext {
            Some(ext) => dir.join(format!("{}__{}.{}", stem, idx, ext)),
            None => dir.join(format!("{}__{}", stem, idx)),
        };
        if !candidate.exists() {
            path = candidate;
            break;
        }
    }

    Ok(path)
}

pub fn copy_to(path: &Path, dest: &Path) -> Result<()> {
    std::fs::copy(path, dest)?;
    Ok(())
}
```

---

## 22. adapter/mod.rs

```rust
pub mod local_folder;
```

## 23. adapter/local_folder.rs

```rust
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::processor::copier;

pub struct LocalFolderAdapter {
    pub root: PathBuf,
}

impl LocalFolderAdapter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn store_copy(&self, source: &Path, source_name: &str) -> Result<PathBuf> {
        let dest = copier::build_dest_path(&self.root, source_name)?;
        copier::copy_to(source, &dest)?;
        Ok(dest)
    }
}
```

---

## 24. agent/mod.rs

```rust
mod scheduler;
mod worker;

use std::{thread, time::Duration};

use anyhow::Result;
use tracing::{debug, info};

use crate::app::App;

pub fn run_agent(app: &App) -> Result<()> {
    info!("agent started");
    loop {
        let processed = scheduler::tick(app)?;
        if !processed {
            thread::sleep(Duration::from_millis(1000));
        } else {
            debug!("agent tick processed work");
        }
    }
}
```

## 25. agent/scheduler.rs

```rust
use anyhow::Result;

use crate::app::App;
use crate::queue::repository::Repository;

use super::worker;

pub fn tick(app: &App) -> Result<bool> {
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);

    if let Some(item) = repo.next_queued_item()? {
        drop(conn);
        worker::process_item(app, item)?;
        return Ok(true);
    }

    Ok(false)
}
```

## 26. agent/worker.rs

```rust
use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info, warn};

use crate::adapter::local_folder::LocalFolderAdapter;
use crate::app::App;
use crate::domain::{ItemJob, ManifestRecord};
use crate::processor::{deduper, hasher, validator};
use crate::queue::repository::Repository;

pub fn process_item(app: &App, item: ItemJob) -> Result<()> {
    let source = PathBuf::from(&item.source_path);
    let target = app.config.default_target()?;

    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    repo.update_item_running(&item.item_id, "validating")?;

    let size = match validator::validate_file(&source, app.config.import.max_file_size_mb) {
        Ok(size) => size,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_SOURCE_INVALID", &err.to_string())?;
            error!(item_id = %item.item_id, error = %err, "validation failed");
            return Ok(());
        }
    };

    let hash = match hasher::sha256_file(&source) {
        Ok(hash) => hash,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_HASH_FAILED", &err.to_string())?;
            error!(item_id = %item.item_id, error = %err, "hash failed");
            return Ok(());
        }
    };
    repo.update_item_hash(&item.item_id, &hash, size as i64)?;

    if let Some(existing_record_id) = deduper::find_duplicate_record(&repo, &item.target_id, &hash)? {
        repo.mark_item_duplicate(&item.item_id, &existing_record_id)?;
        warn!(item_id = %item.item_id, duplicate_of = %existing_record_id, "duplicate skipped");
        return Ok(());
    }

    let adapter = LocalFolderAdapter::new(&target.root_path);
    let dest = match adapter.store_copy(&source, &item.source_name) {
        Ok(dest) => dest,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_COPY_FAILED", &err.to_string())?;
            error!(item_id = %item.item_id, error = %err, "copy failed");
            return Ok(());
        }
    };

    let record = ManifestRecord::new(
        item.item_id.clone(),
        item.target_id.clone(),
        item.source_path.clone(),
        dest.to_string_lossy().to_string(),
        item.source_name.clone(),
        item.file_ext.clone(),
        Some(size as i64),
        hash,
    );
    repo.insert_manifest(&record)?;
    repo.mark_item_success(&item.item_id, &record.stored_path)?;

    info!(item_id = %item.item_id, stored_path = %record.stored_path, "item imported");
    Ok(())
}
```

---

## 27. logging/mod.rs

```rust
use anyhow::Result;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
    Ok(())
}
```

---

## 28. processor/mod.rs / queue/mod.rs

```rust
pub mod scanner;
pub mod validator;
pub mod hasher;
pub mod deduper;
pub mod copier;
```

```rust
pub mod repository;
pub mod state_machine;
```

---

## 29. Windows 右键注册脚本

### scripts/register_file_context_menu.reg

```reg
Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\Classes\*\shell\KBIntake]
@="添加到知识库"

[HKEY_CURRENT_USER\Software\Classes\*\shell\KBIntake\command]
@="\"C:\\Path\\To\\kbintake.exe\" import \"%1\""
```

### scripts/register_dir_context_menu.reg

```reg
Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\Classes\Directory\shell\KBIntake]
@="添加文件夹到知识库"

[HKEY_CURRENT_USER\Software\Classes\Directory\shell\KBIntake\command]
@="\"C:\\Path\\To\\kbintake.exe\" import \"%1\""
```

---

## 30. 第一批先实现顺序

最推荐的开工顺序：

1. `Cargo.toml`
2. `main.rs`
3. `config/mod.rs`
4. `db/schema.rs`
5. `domain/*`
6. `queue/repository.rs`
7. `processor/scanner.rs`
8. `cli/mod.rs` 的 `import`
9. `agent/mod.rs` + `scheduler.rs`
10. `agent/worker.rs`
11. `adapter/local_folder.rs`
12. `jobs list/show`
13. `.reg` 脚本

---

## 31. 你开工后第一轮先验证的命令

```bash
cargo run -- import "C:\Users\me\Desktop\a.pdf"
cargo run -- agent
cargo run -- jobs list
cargo run -- jobs show <batch_id>
```

先别急着做右键，先把 CLI + agent 跑通，再接 Explorer。

---

## 32. 下一步最值得继续补的内容

这套骨架之后，最该继续补的是两样：

1. **把 repository 和 worker 补到“可编译通过”的严格版本**
2. **补一套最小集成测试**

如果你要，我下一条直接给你 **第一版可编译修正版**，把最容易报错的几个文件一次性补平。

