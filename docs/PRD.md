

# KBIntake — Product Requirements Document (v2, Detailed)

### TL;DR

KBIntake is a Windows-native Rust CLI utility (`kbintake.exe`) that enables users to import files and folders into local knowledge-base vaults (e.g., Obsidian) via Explorer right-click context menu or PowerShell commands. It features SHA-256 content deduplication, a SQLite-backed job queue for reliable batch processing, configurable multiple vault targets with file-type routing, and YAML frontmatter injection — all running entirely offline with zero cloud dependency.

---

## Goals

### Business Goals

1. **Establish KBIntake as the go-to open-source CLI tool** for local-first knowledge-base file ingestion on Windows, targeting 500+ GitHub stars within 6 months of public release.
2. **Reduce average file-import friction by 80%** compared to manual copy-paste-rename workflows, measured by user-reported time savings in feedback surveys.
3. **Achieve <1% data-loss incidents** through robust SHA-256 deduplication, atomic file operations, and comprehensive audit logging.
4. **Build a modular architecture** that supports future plugin or extension development (e.g., Logseq, Notion local export) without core rewrites.
5. **Maintain a single-binary distribution model** (no installer dependencies, no runtime requirements) to minimize support overhead.

### User Goals

1. **One-action import:** Right-click any file or folder in Explorer and send it to the correct vault location without navigating folder trees.
2. **Never lose track of imports:** Every import is logged with source path, destination, hash, and timestamp — fully queryable via CLI.
3. **No duplicates:** SHA-256 hashing ensures the same file is never imported twice, even if renamed.
4. **Flexible organization:** Route different file types to different vault locations automatically (e.g., PDFs to `vault/references/`, images to `vault/assets/`).
5. **Safe undo:** Reverse any import batch, restoring the vault to its pre-import state with confidence.

### Non-Goals

1. **Cloud sync or remote storage** — KBIntake is strictly local-first. Syncing is delegated to the user's existing tools (OneDrive, Syncthing, Git, etc.).
2. **Full-text search or indexing** — KBIntake manages file ingestion, not content discovery. Search is handled by the vault application (Obsidian, etc.).
3. **GUI application** — v1.0 is CLI-only. A GUI wrapper is a potential future project but is explicitly out of scope.

---

## User Stories

### Persona 1: Knowledge Worker (Maya)

Maya is a research analyst who collects PDFs, screenshots, and markdown notes from various sources daily. She uses Obsidian as her primary knowledge base.

- As a **Knowledge Worker**, I want to right-click a PDF on my Desktop and send it to my Obsidian vault's `references/` folder, so that I don't have to manually navigate the vault directory.
- As a **Knowledge Worker**, I want duplicate files to be automatically detected and skipped, so that my vault stays clean without me checking manually.
- As a **Knowledge Worker**, I want a toast notification confirming my import succeeded, so that I can continue working without switching to a terminal.
- As a **Knowledge Worker**, I want to undo a recent import if I realize I sent files to the wrong vault, so that I can correct mistakes without manually hunting for files.
- As a **Knowledge Worker**, I want imported markdown files to automatically have frontmatter added with the import date and source, so that I can trace where each note came from.

### Persona 2: Power User / Developer (James)

James is a software developer who maintains multiple Obsidian vaults (personal, work, side-project) and prefers PowerShell automation.

- As a **Power User**, I want to run `kbintake import *.md --target work-vault` from PowerShell, so that I can script bulk imports into my CI/CD or automation workflows.
- As a **Power User**, I want to configure file-type routing rules in a TOML config file, so that PDFs, images, and markdown files automatically land in the correct subfolders.
- As a **Power User**, I want `--json` output on all commands, so that I can pipe results into `jq` or other tools.
- As a **Power User**, I want to query job history with filters (by status, date, target), so that I can audit what was imported and when.
- As a **Power User**, I want a `--dry-run` flag, so that I can preview exactly what an import would do before committing.

### Persona 3: Non-Developer (Sara)

Sara is a teacher who uses Obsidian to organize lesson plans and student resources. She is not comfortable with terminals but can follow setup instructions.

- As a **Non-Developer**, I want a simple right-click menu option that says "Send to KBIntake", so that I don't have to learn command-line syntax.
- As a **Non-Developer**, I want clear error messages in plain English if something goes wrong, so that I can fix the problem or ask for help.
- As a **Non-Developer**, I want a `kbintake doctor` command that checks everything is set up correctly, so that I can verify my installation without technical knowledge.

---

## Functional Requirements

### Group 1 — File Import Engine (Priority: P0)

- **Single & Multi-File Import:** Accept one or more file/folder paths as arguments. Recursively traverse folders. Symlinks are NOT followed (logged as skipped).
- **SHA-256 Deduplication:** Before copying any file, compute its SHA-256 hash and compare against all existing hashes in the target vault's item records. If a match is found, mark the item as `duplicate` and skip the copy. The hash is stored in the `items` table for future dedup checks.
- **Atomic Copy:** Files are first copied to a temporary location within the target vault (`_kbintake_tmp/`), then renamed to the final destination. If the rename fails, the temp file is cleaned up and the item is marked `failed`.
- **Max File Size Limit:** Configurable via `max_file_size_mb` in `config.toml`. Default is `0` (unlimited). When set to a positive integer, any file exceeding the limit is **not copied**. The item is logged with status `failed`, error message includes the file size and limit, and the CLI prints a `WARN` to stderr. If **all** files in a batch exceed the limit, the process exits with code `3`. If some files exceed and others succeed, the batch is marked `partially_failed` and exits with code `6`.
- **File-Type Routing:** If `[[routing.rules]]` entries exist in `config.toml`, files matching a rule's `extension` are routed to the specified target, overriding the default or `--target` flag. If the routing target does not exist, the item fails with exit code `4`.
- **Frontmatter Injection (Markdown only):** For `.md` files, prepend a YAML frontmatter block if one does not already exist. Fields (see Group 6 for full spec): `kb_imported_at` (ISO 8601), `kb_source_path` (original absolute path), `kb_batch_id` (UUID), `kb_sha256` (hash). If frontmatter already exists, append KBIntake fields without overwriting existing fields.
- **Queue vs. Immediate Processing:** By default, behavior is controlled by `queue_only` in `config.toml` (default `false`, meaning process immediately). The `--queue-only` flag overrides to queue-only. The `--process` flag overrides to process immediately. Explicit flags always take precedence over config.
- **Dry-Run Mode:** When `--dry-run` is passed, perform all validation (path checks, hash computation, dedup comparison, size limit checks, routing resolution) but do **not** copy any files or write to the database. Output a preview table to stdout showing each file with columns: `Source Path`, `Destination`, `Action` (copy / skip-duplicate / skip-size-limit / skip-symlink). Exit code `0` regardless of what would have happened.

### Group 2 — Job Queue & Batch Management (Priority: P0)

- **Batch Creation:** Every invocation of `kbintake import` creates exactly one batch record in the `batches` table. All files from that invocation are items within that batch.
- **Jobs List:** `kbintake jobs list` shows recent batches. Default: last 20, sorted by `created_at` descending. Supports `--status` filter (e.g., `--status failed`), `--limit`, and `--json` / `--table` output format. Table format is the default.
- **Jobs Show:** `kbintake jobs show <batch-id>` displays batch details and all items within it, including per-item status, source path, destination path, and error messages if any.
- **Jobs Retry:** `kbintake jobs retry <batch-id>` re-processes all items with status `failed` within the specified batch. Items with other statuses are untouched. If no failed items exist, print an info message and exit `0`.
- **Jobs Undo:** `kbintake jobs undo <batch-id>` reverses an import batch. Behavioral rules:
  1. For each item with status `success` in the batch, re-read the destination file and compute its SHA-256 hash.
  2. Compare the current hash to the `sha256` value recorded at import time.
  3. **If hashes match:** Delete the destination file. Update item status to `undone`.
  4. **If hashes differ:** The file has been modified since import. By default, **skip** this file. Print `WARN: File '{dest_path}' skipped during undo — content has been modified since import.` to stderr. Update item status to `undo_skipped_modified`.
  5. After processing all items, print a summary: `Undo complete: {X} deleted, {Y} skipped (modified).`
  6. If all items were successfully undone: batch status → `undone`.
  7. If any items were skipped: batch status → `partially_undone`. Exit code `6`.
  8. **`--force` flag:** If provided, delete files even when hashes differ. Print a `WARN` per file noting the modification, but proceed with deletion. All items become `undone`, batch status → `undone`.
  9. Items with status `duplicate` are skipped during undo (nothing was copied). Items with status `failed` are skipped (nothing to undo).

### Group 3 — Target (Vault) Management (Priority: P0)

- **Targets Add:** `kbintake targets add <name> <path>` registers a new vault target. The `path` must be an existing directory; if not, exit with code `2` and message. Name must be unique among active targets. If this is the first target, automatically set it as default.
- **Targets List:** Show all active targets with name, path, and default indicator. Archived targets are hidden by default. `--include-archived` flag shows all targets with a status column.
- **Targets Show:** Display full details for a single target, including stats (total items imported, total batches, disk usage).
- **Targets Rename:** `kbintake targets rename <old> <new>` updates the target name. Rejects if `<old>` is archived (exit code `5`, message: `Target '{old}' is archived and cannot be used.`). Rejects if `<new>` name already exists.
- **Targets Set-Default:** `kbintake targets set-default <name>` sets the default target. Rejects if target is archived (exit code `5`).
- **Targets Remove:** `kbintake targets remove <name>` removes a target. Behavioral rules:
  1. If the target has any items with status `queued` (pending/unprocessed jobs), **reject** the removal. Exit code `5`, message: `Cannot remove target '{name}' — {n} pending job(s) exist. Process or cancel them first.`
  2. If the target has only completed, failed, or other terminal-status historical jobs, **archive** the target: set `status = 'archived'` and `updated_at` to current timestamp. Do NOT delete the database row.
  3. If the target was the default, clear the default (no target is default; user must set a new one).
  4. `--force` flag: Skip the pending-jobs check and archive anyway. Print a `WARN` noting that `{n}` queued items will be orphaned.

### Group 4 — Explorer Context Menu Integration (Priority: P0)

- **Install:** `kbintake explorer install` writes the necessary registry keys under `HKEY_CURRENT_USER\Software\Classes\*\shell\KBIntake` and `HKEY_CURRENT_USER\Software\Classes\Directory\shell\KBIntake` to add a "Send to KBIntake" context menu entry for files and folders. The registry command points to `kbintake.exe import "%1"` (with `--toast` internal flag to enable toast notifications).
- **Uninstall:** `kbintake explorer uninstall` removes the registry keys. Idempotent — no error if keys don't exist.
- **No elevation required:** Uses `HKCU` (current user) registry hive, so no administrator privileges are needed.

### Group 5 — Configuration Management (Priority: P1)

- **Config Location:** `%LOCALAPPDATA%\kbintake\config.toml`. Created with sensible defaults on first run or via `kbintake doctor`.
- **Config Show:** `kbintake config show` prints the current parsed configuration to stdout in TOML format.
- **Config Set:** `kbintake config set <key> <value>` updates a single config key. Supports dotted keys (e.g., `general.max_file_size_mb 100`). Validates the value type before writing. See the **config.toml Full Specification** section below for all fields.

### Group 6 — Frontmatter Injection (Priority: P1)

- **Applicable files:** Only `.md` files. All other file types are copied without modification.
- **Frontmatter fields (injected by KBIntake):**
  - `kb_imported_at`: ISO 8601 timestamp of import (e.g., `2025-01-15T09:30:00+08:00`)
  - `kb_source_path`: Original absolute file path on the source machine (e.g., `C:\Users\Maya\Desktop\notes.md`)
  - `kb_batch_id`: UUID of the import batch
  - `kb_sha256`: SHA-256 hash of the **original** file content (before frontmatter injection)
- **Behavior with existing frontmatter:** If the file already starts with `---`, parse the existing frontmatter block. Append KBIntake fields at the end of the block (before the closing `---`). Never overwrite existing user fields. If a `kb_` prefixed field already exists, overwrite it (idempotent re-import).
- **SHA-256 calculation order:** Hash is computed on the **original file bytes** before any frontmatter modification. The hash stored in the database and injected into frontmatter refers to the pre-modification content.

### Group 7 — Audit & Diagnostics (Priority: P1)

- **Audit Events Table:** Every significant action (import, undo, retry, target add/remove/archive) is logged in the `audit_events` table with event type, related IDs, and a JSON payload for structured details.
- **Doctor Command:** `kbintake doctor` performs a system health check:
  1. Verify `config.toml` exists and is valid TOML.
  2. Verify SQLite database file exists and schema version matches current expected version.
  3. Verify all active targets point to existing directories.
  4. Verify Explorer context menu registry keys are present (if installed).
  5. Print a summary: `✓ Config OK`, `✓ Database OK`, `✗ Target 'work' path not found: D:\vault`, etc.
  6. Exit code `0` if all checks pass, `1` if any fail.
- **Vault Stats:** `kbintake vault stats [--target <name>]` shows aggregate statistics: total files imported, total duplicates skipped, total disk usage by target, date range of imports.

### Group 8 — Background Service (Priority: P2)

- **Service Mode:** `kbintake service start` launches a long-running process that polls the `batches` table every `poll_interval_secs` (default 30) for queued batches and processes them sequentially.
- **Service Stop:** `kbintake service stop` sends a graceful shutdown signal.
- **Implementation:** Uses a simple polling loop, not a Windows Service. Designed to run as a startup task via Task Scheduler or shortcut in the Startup folder.

### Group 9 — Extensibility Hooks (Priority: P2)

- **Post-Import Hook:** If a `post_import` script path is configured in `config.toml`, execute it after each batch completes. Pass the batch ID as the first argument. The hook is fire-and-forget; its exit code does not affect KBIntake's exit code.

---

## config.toml Full Specification

The configuration file is located at `%LOCALAPPDATA%\kbintake\config.toml`. Below is the complete annotated structure:

```toml
# ============================================================
# KBIntake Configuration File
# Location: %LOCALAPPDATA%\kbintake\config.toml
# ============================================================

[general]
# The name of the default vault target.
# Must match a target registered via 'kbintake targets add'.
# Type: string (target name)
# Default: "" (empty — user must set via 'kbintake targets set-default')
default_target = "personal-vault"

# Maximum file size in megabytes allowed for import.
# Files exceeding this limit are skipped and logged as 'failed'.
# Set to 0 for unlimited (no size restriction).
# Type: integer
# Default: 0
max_file_size_mb = 0

# If true, 'kbintake import' only queues batches without processing.
# Processing must be triggered manually ('kbintake jobs retry') or by the service.
# CLI flags --process and --queue-only override this setting.
# Type: boolean
# Default: false
queue_only = false

[service]
# How often (in seconds) the background service polls for queued batches.
# Only relevant when running 'kbintake service start'.
# Type: integer
# Default: 30
poll_interval_secs = 30

# ============================================================
# Routing Rules
# Route specific file extensions to specific targets automatically.
# If a file matches a rule, it overrides the default target.
# Rules are evaluated in order; first match wins.
# ============================================================

[[routing.rules]]
# File extension to match (include the dot).
# Type: string
extension = ".pdf"
# Target name to route matching files to.
# Must be a registered, active target.
# Type: string
target = "references-vault"

[[routing.rules]]
extension = ".png"
target = "assets-vault"

[[routing.rules]]
extension = ".jpg"
target = "assets-vault"

[[routing.rules]]
extension = ".mp4"
target = "media-vault"

# ============================================================
# Hooks (optional)
# ============================================================

[hooks]
# Path to a script executed after each batch completes.
# Receives the batch UUID as the first argument.
# Type: string (file path) or "" to disable.
# Default: ""
post_import = ""
```

| Field | Type | Default | Description |
|---|---|---|---|
| `general.default_target` | string | `""` | Name of the default vault target |
| `general.max_file_size_mb` | integer | `0` | Max file size in MB; 0 = unlimited |
| `general.queue_only` | boolean | `false` | Queue-only mode; CLI flags override |
| `service.poll_interval_secs` | integer | `30` | Service polling interval in seconds |
| `routing.rules[].extension` | string | — | File extension to match (e.g., `.pdf`) |
| `routing.rules[].target` | string | — | Target name for matched files |
| `hooks.post_import` | string | `""` | Path to post-import hook script |

---

## SQLite Database Schema

The database file is located at `%LOCALAPPDATA%\kbintake\kbintake.db`. All timestamps are stored as ISO 8601 strings in UTC.

### Table: `schema_version`

Tracks the current database schema version. Contains exactly one row. At startup, KBIntake reads this table; if the version is lower than the application's expected version, it runs incremental migrations before proceeding.

| Column | Type | Constraints | Description |
|---|---|---|---|
| `version` | INTEGER | NOT NULL | Current schema version integer |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp of last migration |

### Table: `targets`

Stores registered vault targets (destinations for imported files).

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `name` | TEXT | UNIQUE NOT NULL | Human-readable target name |
| `path` | TEXT | NOT NULL | Absolute filesystem path to vault root |
| `is_default` | INTEGER | NOT NULL, DEFAULT 0 | `1` if this is the default target, `0` otherwise. Only one row should have `1` at any time. |
| `status` | TEXT | NOT NULL, DEFAULT 'active' | `active` or `archived` |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp |

**Constraints:** Application-level enforcement ensures at most one row has `is_default = 1`. When archiving a target, `status` is set to `archived` and the row is retained.

### Table: `batches`

Represents a single import invocation (one CLI call = one batch).

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |
| `status` | TEXT | NOT NULL, DEFAULT 'queued' | One of: `queued`, `processing`, `completed`, `failed`, `partially_failed`, `undone`, `partially_undone` |
| `item_count` | INTEGER | NOT NULL | Total number of items in this batch |
| `source_note` | TEXT | NULLABLE | Optional user annotation or invocation context |

**Status transitions:**
- `queued` → `processing` → `completed` | `failed` | `partially_failed`
- `completed` | `partially_failed` → `undone` | `partially_undone` (via undo)

### Table: `items`

Individual files within a batch.

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `batch_id` | TEXT | FK → `batches.id`, NOT NULL | Parent batch |
| `source_path` | TEXT | NOT NULL | Original absolute path of the source file |
| `dest_path` | TEXT | NULLABLE | Absolute path in the target vault (null if never copied) |
| `target_id` | TEXT | FK → `targets.id`, NOT NULL | Target vault this item was routed to |
| `sha256` | TEXT | NOT NULL | SHA-256 hash of original file content |
| `file_size_bytes` | INTEGER | NOT NULL | File size in bytes |
| `status` | TEXT | NOT NULL, DEFAULT 'queued' | One of: `queued`, `success`, `duplicate`, `failed`, `skipped`, `undone`, `undo_skipped_modified` |
| `error_message` | TEXT | NULLABLE | Error description if status is `failed` or `undo_skipped_modified` |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp |

### Table: `audit_events`

Append-only log of all significant system actions.

| Column | Type | Constraints | Description |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `item_id` | TEXT | FK → `items.id`, NULLABLE | Related item (if applicable) |
| `batch_id` | TEXT | FK → `batches.id`, NULLABLE | Related batch (if applicable) |
| `event_type` | TEXT | NOT NULL | Event category: `import`, `duplicate_skipped`, `undo`, `undo_skipped`, `retry`, `target_added`, `target_archived`, `target_renamed`, `config_changed`, `error` |
| `payload` | TEXT | NULLABLE | JSON object with event-specific details |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp |

---

## CLI Command Reference

### `kbintake import <paths...>`

**Description:** Import one or more files or folders into a vault target.

| Argument/Flag | Required | Description |
|---|---|---|
| `<paths...>` | Yes | One or more file or directory paths |
| `--target <name>` | No | Override default target; also overridden by routing rules |
| `--process` | No | Process immediately (override `queue_only` config) |
| `--queue-only` | No | Queue only, do not process now |
| `--dry-run` | No | Preview actions without writing to filesystem or database |

**Example:**
```
kbintake import "C:\Users\Maya\report.pdf" "C:\Users\Maya\notes.md" --target personal
```

**Stdout:** Progress lines per file. Final summary: `Imported: {n}, Duplicates: {d}, Failed: {f}`. With `--dry-run`, a preview table. With `--json`, a JSON array of item results.

**Exit codes:** `0` (all success), `3` (all failed due to size limit), `6` (partial success), `1` (unexpected error), `2` (missing config/args), `4` (target not found).

---

### `kbintake jobs list`

**Description:** List recent import batches.

| Flag | Required | Description |
|---|---|---|
| `--json` | No | Output as JSON array |
| `--table` | No | Output as formatted table (default) |
| `--status <status>` | No | Filter by batch status |
| `--limit <n>` | No | Max number of results (default: 20) |

**Example:**
```
kbintake jobs list --status failed --limit 5
```

**Exit codes:** `0` (success), `8` (database error).

---

### `kbintake jobs show <batch-id>`

**Description:** Show details of a specific batch and all its items.

| Argument/Flag | Required | Description |
|---|---|---|
| `<batch-id>` | Yes | UUID of the batch |
| `--json` | No | Output as JSON |

**Example:**
```
kbintake jobs show a1b2c3d4-e5f6-7890-abcd-ef1234567890
```

**Exit codes:** `0` (found), `2` (invalid/missing batch ID), `8` (database error).

---

### `kbintake jobs retry <batch-id>`

**Description:** Re-process all failed items within a batch.

**Example:**
```
kbintake jobs retry a1b2c3d4-e5f6-7890-abcd-ef1234567890
```

**Exit codes:** `0` (all retried successfully), `6` (some still failed), `2` (invalid batch ID), `8` (database error).

---

### `kbintake jobs undo <batch-id>`

**Description:** Reverse an import batch by deleting imported files from the vault.

| Flag | Required | Description |
|---|---|---|
| `--force` | No | Delete files even if they have been modified since import |

**Example:**
```
kbintake jobs undo a1b2c3d4-e5f6-7890-abcd-ef1234567890
kbintake jobs undo a1b2c3d4-e5f6-7890-abcd-ef1234567890 --force
```

**Stdout:** Per-file status line, then summary: `Undo complete: {X} deleted, {Y} skipped (modified).`

**Exit codes:** `0` (all undone), `6` (partially undone — some files modified), `2` (invalid batch ID), `8` (database error).

---

### `kbintake targets add <name> <path>`

**Description:** Register a new vault target.

**Example:**
```
kbintake targets add personal "D:\Obsidian\PersonalVault"
```

**Exit codes:** `0` (success), `2` (path does not exist or name already taken), `8` (database error).

---

### `kbintake targets list`

**Description:** List all registered vault targets.

| Flag | Required | Description |
|---|---|---|
| `--include-archived` | No | Include archived targets in output |

**Example:**
```
kbintake targets list
kbintake targets list --include-archived
```

**Exit codes:** `0` (success), `8` (database error).

---

### `kbintake targets show <name>`

**Description:** Show detailed information about a specific target, including import statistics.

**Exit codes:** `0` (found), `4` (target not found), `8` (database error).

---

### `kbintake targets rename <old> <new>`

**Description:** Rename a target. Rejects archived targets.

**Exit codes:** `0` (success), `4` (old name not found), `5` (target is archived), `2` (new name already exists), `8` (database error).

---

### `kbintake targets remove <name>`

**Description:** Archive a target (soft-delete).

| Flag | Required | Description |
|---|---|---|
| `--force` | No | Skip pending-jobs check |

**Exit codes:** `0` (archived), `5` (has pending jobs), `4` (target not found), `8` (database error).

---

### `kbintake targets set-default <name>`

**Description:** Set the default vault target. Rejects archived targets.

**Exit codes:** `0` (success), `4` (not found), `5` (archived), `8` (database error).

---

### `kbintake config show`

**Description:** Print current parsed configuration in TOML format to stdout.

**Exit codes:** `0` (success), `2` (config file not found or invalid).

---

### `kbintake config set <key> <value>`

**Description:** Update a single configuration value.

**Example:**
```
kbintake config set general.max_file_size_mb 100
kbintake config set general.queue_only true
```

**Exit codes:** `0` (success), `2` (invalid key or value type).

---

### `kbintake explorer install` / `kbintake explorer uninstall`

**Description:** Add or remove the Windows Explorer right-click context menu entry.

**Exit codes:** `0` (success), `1` (registry write error).

---

### `kbintake doctor`

**Description:** Run a system health check on config, database, targets, and Explorer integration.

**Example:**
```
kbintake doctor
```

**Stdout:** Checklist with `✓` / `✗` per check. **Exit codes:** `0` (all pass), `1` (any check fails).

---

### `kbintake vault stats`

**Description:** Show aggregate vault statistics.

| Flag | Required | Description |
|---|---|---|
| `--target <name>` | No | Limit stats to a specific target |

**Exit codes:** `0` (success), `4` (target not found), `8` (database error).

---

## Exit Code Reference

| Code | Name | Description |
|---|---|---|
| `0` | Success | Operation completed successfully |
| `1` | General Error | Unexpected/unhandled error |
| `2` | Invalid Arguments | Missing required arguments, invalid config, or validation failure |
| `3` | File Size Exceeded | All files in batch exceeded `max_file_size_mb` limit |
| `4` | Target Not Found | Specified target name does not exist in the database |
| `5` | Operation Rejected | Constraint violation (e.g., pending jobs on target removal, target is archived) |
| `6` | Partial Success | Some items succeeded, some failed; or partial undo (modified files skipped) |
| `7` | Duplicate Detected | Reserved for future `--fail-on-duplicate` flag |
| `8` | Database Error | SQLite read/write failure |

**Rules:**
- All error and warning messages are written to **stderr**.
- All structured output (`--json`) and normal output is written to **stdout**.
- Exit code `6` is used for both `partially_failed` and `partially_undone` batch outcomes.

---

## Toast Notification Specification

Toast notifications are **only** shown when the import is triggered from the Explorer right-click context menu (internal `--toast` flag). They are never shown for direct CLI invocations. All notifications in v1.0 are non-interactive (no action buttons).

| Scenario | Title | Body |
|---|---|---|
| **Full Success** | `KBIntake — Import Complete` | `{n} file(s) imported to {target_name}.` If `d > 0`, append: `{d} duplicate(s) skipped.` |
| **Partial Failure** | `KBIntake — Import Completed with Errors` | `{n} file(s) imported. {f} failed. Run: kbintake jobs retry {batch_id}` |
| **Full Failure** | `KBIntake — Import Failed` | `All {n} file(s) failed. Run: kbintake jobs retry {batch_id}` |
| **Undo Success** | `KBIntake — Undo Complete` | `{n} file(s) removed from vault.` |
| **Partial Undo** | `KBIntake — Undo Partially Complete** | `{n} removed, {s} skipped (modified since import). Check: kbintake jobs show {batch_id}` |

---

## Error Message Catalog

All error messages follow the format `ERROR [{code}]: {message}` on stderr. Warnings use `WARN: {message}`.

| Code | Message |
|---|---|
| `2` | `No default target configured. Run: kbintake targets set-default <name>` |
| `2` | `Config file not found at %LOCALAPPDATA%\kbintake\config.toml. Run: kbintake doctor` |
| `2` | `Invalid argument: {detail}` |
| `2` | `Target path '{path}' does not exist. Provide a valid directory.` |
| `3` | `File '{path}' exceeds max_file_size_mb limit ({size}MB > {limit}MB). Skipped.` |
| `4` | `Target '{name}' not found. Use: kbintake targets list` |
| `5` | `Cannot remove target '{name}' — {n} pending job(s) exist. Process or cancel them first.` |
| `5` | `Target '{name}' is archived and cannot be used.` |
| `8` | `Database error: {sqlite_error_message}` |
| WARN | `File '{path}' skipped during undo — content has been modified since import.` |
| WARN | `File '{path}' exceeds max_file_size_mb limit ({size}MB > {limit}MB), skipping.` |
| WARN | `Target '{name}' had {n} queued item(s) — forced archive.` |

---

## User Experience

**Entry Point & First-Time User Experience**

- User downloads `kbintake.exe` (single binary, no installer) and places it in a directory on their PATH (e.g., `C:\Tools\`).
- On first run of any command, KBIntake checks for `%LOCALAPPDATA%\kbintake\config.toml`. If missing, it creates the directory and a default config file with empty `default_target` and all other defaults.
- The SQLite database (`kbintake.db`) is created in the same directory with the schema initialized to the current version.
- User runs `kbintake targets add personal "D:\MyVault"` to register their first vault. Since it's the first target, it is automatically set as default.
- User runs `kbintake explorer install` to add the right-click context menu.
- User runs `kbintake doctor` to verify everything is configured correctly.
- The entire setup process takes under 2 minutes and requires no elevated permissions.

**Core Experience**

- **Step 1: User selects files in Explorer and right-clicks → "Send to KBIntake"**
  - Windows Explorer invokes `kbintake.exe import "{path}" --toast`.
  - The CLI immediately validates: config file exists, default target (or routed target) is set and active, target path exists on disk.
  - If validation fails, a toast notification is shown with the error message and suggested fix command.

- **Step 2: Import processing begins**
  - A new batch record is created in the database.
  - For each file: compute SHA-256 hash → check size limit → check for duplicates → determine target (routing rules or default) → copy to vault via atomic temp-rename pattern.
  - For `.md` files, frontmatter is injected after copying (injected into the destination copy, not the source).
  - Progress is logged per-item to the database.

- **Step 3: User receives feedback**
  - A Windows toast notification appears with the outcome (success, partial failure, or full failure).
  - The notification includes the count of files imported and, if applicable, the `batch_id` for retry.
  - No further user action needed for successful imports.

- **Step 4: User queries import history (optional)**
  - `kbintake jobs list` shows recent batches.
  - `kbintake jobs show <batch-id>` drills into a specific batch.
  - `kbintake vault stats` provides aggregate insights.

- **Step 5: User undoes a mistaken import (optional)**
  - `kbintake jobs undo <batch-id>` safely removes imported files, with hash verification to protect user edits.

**Advanced Features & Edge Cases**

- **No default target and `--target` not provided:** Exit code `2`. Error message: `No default target configured. Run: kbintake targets set-default <name>`. Toast notification (if `--toast`): same message.
- **Vault path does not exist at import time:** Each item targeting that vault is marked `failed` with error `Target path '{path}' does not exist`. If all items fail, batch status is `failed`. If mixed, batch is `partially_failed`.
- **Disk full mid-import:** The atomic copy to `_kbintake_tmp/` will fail with an OS-level I/O error. The error is caught per-item; remaining items in the batch are attempted but will likely also fail. Batch is marked `partially_failed` or `failed`. Error message includes the OS error string.
- **Symlinks:** Symlinked files and folders are not followed. They are logged as `skipped` with error message `Symlink not supported`.
- **File locked by another process:** Copy fails with OS error. Item is marked `failed`. User can retry later with `kbintake jobs retry`.
- **`--dry-run`:** Outputs a preview table with columns: `Source`, `Destination`, `Action` (`copy`, `skip-duplicate`, `skip-size-limit`, `skip-symlink`). No files are copied, no database records are created. Exit code is always `0`.
- **Concurrent invocations:** SQLite WAL mode is enabled for safe concurrent reads. Writes are serialized by SQLite's internal locking. Two simultaneous imports will each create their own batch and process independently.

**UI/UX Highlights**

- **Zero-GUI philosophy:** All user feedback is through CLI stdout/stderr or Windows toast notifications. No windows, dialogs, or prompts.
- **Color-coded terminal output:** Use ANSI colors when the terminal supports it (`--no-color` flag to disable). Green for success, yellow for warnings/duplicates, red for errors.
- **Table formatting:** Default output uses aligned ASCII tables (compatible with all terminals). `--json` for machine-readable output.
- **Consistent flag naming:** All flags use lowercase kebab-case (`--dry-run`, `--queue-only`, `--include-archived`).
- **Accessible error messages:** Every error message includes a suggested remediation command. Users should never see a raw stack trace.

---

## Narrative

Maya is a research analyst at a consulting firm, and her workflow generates a relentless stream of PDFs, screenshots, and quick markdown notes throughout the day. Her Obsidian vault is her second brain — but the manual process of dragging files into the right subfolder, renaming them, and avoiding duplicates eats into her focus time. Some days she saves the same report twice without realizing it, cluttering her vault and confusing her Dataview queries.

After installing KBIntake, Maya spends two minutes registering her vault as a target and enabling the right-click context menu. Now, when she downloads a client report, she right-clicks it and selects "Send to KBIntake." The PDF is automatically routed to `vault/references/`, deduplicated against her existing library, and logged with a full audit trail. A small toast notification confirms "1 file imported to personal-vault" — and Maya never leaves her browser.

When she accidentally imports an entire folder of drafts to the wrong vault, she pulls up the batch ID from `kbintake jobs list`, runs `kbintake jobs undo`, and the files are cleanly removed. Her vault stays pristine, her Dataview queries stay accurate, and she reclaims the 15 minutes a day she used to spend on file management. That time now goes toward the analysis her clients actually pay for.

---

## Success Metrics

### User-Centric Metrics

- **Import success rate:** ≥ 99% of import operations complete without errors (measured from `batches` table: `completed` / total).
- **Time-to-import:** Average wall-clock time from right-click to toast notification < 3 seconds for batches of ≤ 10 files.
- **Undo usage rate:** Track how often `jobs undo` is invoked as a proxy for user mistakes — target < 5% of batches are undone.
- **Doctor pass rate:** ≥ 95% of `kbintake doctor` runs pass all checks (indicates healthy installations).

### Business Metrics

- **GitHub stars:** 500+ within 6 months of public release.
- **Active installations:** 200+ unique machines running `kbintake doctor` or `import` at least once per month (measured via opt-in anonymous telemetry, if added in the future — out of scope for v1.0).
- **Issue resolution time:** Median time from GitHub issue open to close < 7 days.

### Technical Metrics

- **Binary size:** < 10 MB for `kbintake.exe`.
- **Memory usage:** < 50 MB RSS during a 100-file import batch.
- **SHA-256 throughput:** ≥ 500 MB/s hashing speed on modern hardware (leveraging `sha2` crate's optimized implementation).
- **Database operation latency:** Single row insert/update < 1 ms on SSD.

### Tracking Plan

- `import_batch_created` — Logged per batch: item count, target name, invocation source (CLI vs. Explorer).
- `import_item_processed` — Logged per item: status (success/duplicate/failed/skipped), file size, elapsed time.
- `undo_batch_executed` — Logged per undo: items deleted, items skipped, `--force` used.
- `target_added` / `target_archived` / `target_renamed` — Logged per target management action.
- `doctor_executed` — Logged with pass/fail result per check.
- `config_changed` — Logged with key name and new value (not sensitive values).
- `explorer_installed` / `explorer_uninstalled` — Logged per context menu change.

All tracking data is stored locally in the `audit_events` table. No data is transmitted externally.

---

## Technical Considerations

### Technical Needs

**Recommended Rust Crates:**

| Category | Crate | Notes |
|---|---|---|
| CLI parsing | `clap` v4 | Derive-based argument parsing with subcommands |
| SQLite | `rusqlite` with `bundled` feature | Bundles SQLite into the binary; no external DLL needed |
| SHA-256 | `sha2` | Part of RustCrypto; hardware-accelerated on modern CPUs |
| TOML parsing | `toml` | Serde-based deserialization for config |
| UUID generation | `uuid` with `v4` feature | For all primary keys |
| Windows notifications | `tauri-winrt-notification` or `windows-rs` | WinRT toast API |
| YAML frontmatter | Manual string prepend | No crate needed; simple string concatenation |
| Timestamps | `chrono` | ISO 8601 formatting with timezone support |
| Colored output | `colored` or `termcolor` | ANSI color support for terminal output |

**Architecture:**
- Single binary, no dynamic dependencies beyond Windows system DLLs.
- Modular internal structure: `cli` (argument parsing), `engine` (import/undo logic), `db` (SQLite operations), `config` (TOML management), `notify` (toast notifications), `registry` (Explorer integration).
- All exit codes defined as constants in a shared `exit_codes.rs` module.

### Integration Points

- **Windows Explorer:** Registry keys in `HKCU\Software\Classes\*\shell\KBIntake` and `HKCU\Software\Classes\Directory\shell\KBIntake`.
- **Obsidian vaults:** KBIntake writes to the vault's filesystem. Obsidian's file watcher automatically picks up new files. No Obsidian plugin API is used.
- **Task Scheduler (optional):** For running `kbintake service start` on login.
- **PowerShell / cmd.exe:** Primary CLI invocation environments.

### Data Storage & Privacy

- **All data is local.** Database, config, and vault files never leave the user's machine.
- **No telemetry** in v1.0. If added in the future, it must be opt-in with clear disclosure.
- **Sensitive paths:** `source_path` in the database contains full filesystem paths. Users should be aware if they share their database file.
- **Database location:** `%LOCALAPPDATA%\kbintake\kbintake.db` — protected by Windows user-level file permissions.
- **No encryption** of the database in v1.0. The file is readable by any process running as the same user.

### Scalability & Performance

- **Target scale:** Designed for individual users with vaults containing up to ~100,000 files and import histories of ~50,000 batches.
- **SHA-256 hashing:** Streaming hash (read file in chunks) to avoid loading large files into memory. Expected throughput ≥ 500 MB/s on NVMe SSDs.
- **SQLite performance:** WAL mode enabled for concurrent read access. Index on `items.sha256` for fast dedup lookups. Index on `items.batch_id` for batch queries. Index on `batches.status` for job list filtering.
- **Batch size:** No hard limit on items per batch, but CLI output is paginated for batches with > 100 items.

### Potential Challenges

- **Toast notification AppUserModelID:** Windows requires the sending application to have an Application User Model ID (AUMID) registered for toast notifications to display correctly. The `explorer install` command should also register the AUMID in the registry (`HKCU\Software\Classes\AppUserModelId\KBIntake.CLI`). If using `tauri-winrt-notification`, the crate handles AUMID registration internally — verify this during development.
- **Explorer context menu on Windows 11:** Windows 11 uses a new context menu by default; the classic `Shell\KBIntake` keys appear under "Show more options." Documenting this limitation is sufficient for v1.0. A future version could register as an `IExplorerCommand` COM object for native Windows 11 support.
- **File path length:** Windows has a 260-character path limit by default. If the destination path exceeds this, the copy will fail with an OS error. KBIntake should prefix paths with `\\?\` for extended-length path support.
- **Antivirus interference:** Some antivirus software may flag rapid file creation in a watched folder. Document this as a known issue with mitigation (whitelist `kbintake.exe` and the vault directory).
- **SQLite WAL file growth:** Under heavy write load, the WAL file can grow large. KBIntake should run `PRAGMA wal_checkpoint(TRUNCATE)` periodically (e.g., after every batch completion).

---

## Milestones & Sequencing

### Project Estimate

**Medium: 3–4 weeks** for a single experienced developer (or a developer with strong Rust guidance).

### Team Size & Composition

**Small Team: 1–2 people total**

- 1 Rust Developer (primary) — implements all CLI, engine, database, and registry logic.
- 1 Product/QA (part-time, ~25%) — reviews PRD compliance, writes test scenarios, performs manual testing on Windows.

### Before Development Starts (Pre-Phase Checklist)

The developer must confirm the following before writing any feature code:

- [ ] Rust toolchain installed: stable channel, `x86_64-pc-windows-msvc` target.
- [ ] Project initialized with `cargo init`, `Cargo.toml` configured with all required dependencies (clap, rusqlite, sha2, toml, uuid, chrono, colored).
- [ ] SQLite schema implemented per this spec, including `schema_version` table with version `1`.
- [ ] `config.toml` struct defined and validated against this spec (serde deserialization with defaults).
- [ ] All exit codes implemented as named constants in `src/exit_codes.rs`.
- [ ] Error message catalog implemented as a centralized module (`src/errors.rs`) with all messages from this spec.
- [ ] CI pipeline set up: `cargo build --release`, `cargo test`, `cargo clippy`.

### Suggested Phases

**Phase 1 — Core Engine (1.5 weeks)**

- Key Deliverables (Rust Developer):
  - SQLite database initialization with full schema and migration support.
  - `config.toml` parsing with validation and defaults.
  - File import engine: SHA-256 hashing, dedup check, atomic copy, frontmatter injection.
  - Batch and item lifecycle management in the database.
  - `kbintake import` command with `--target`, `--process`, `--queue-only`, `--dry-run` flags.
  - `kbintake jobs list`, `jobs show`, `jobs retry`, `jobs undo` (with hash-check and `--force`).
  - `kbintake targets add`, `targets list`, `targets show`, `targets rename`, `targets remove`, `targets set-default`.
  - `kbintake config show`, `config set`.
  - `kbintake doctor`.
  - All exit codes and error messages per spec.
- Dependencies: Pre-phase checklist completed.

**Phase 2 — Windows Integration (1 week)**

- Key Deliverables (Rust Developer):
  - `kbintake explorer install` / `uninstall` — registry key creation/removal.
  - Toast notification system — AUMID registration, all 5 notification scenarios implemented.
  - `kbintake vault stats` command.
  - `--json` output format for all commands.
  - Color-coded terminal output with `--no-color` support.
  - End-to-end testing on Windows 10 and Windows 11.
- Dependencies: Phase 1 complete.

**Phase 3 — Polish & Release (0.5–1 week)**

- Key Deliverables (Rust Developer + Product/QA):
  - Background service (`kbintake service start/stop`) — basic polling loop.
  - Post-import hook support.
  - README.md with installation instructions, quickstart guide, and full command reference.
  - `CHANGELOG.md` for v1.0.0.
  - Binary build and GitHub Release with `kbintake.exe` artifact.
  - QA: Test all error scenarios, edge cases (large files, locked files, missing targets, concurrent imports), and undo with modified files.
- Dependencies: Phase 2 complete.
