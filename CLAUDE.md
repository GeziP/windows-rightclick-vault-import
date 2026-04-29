# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**KBIntake** is a Windows-first local vault importer CLI (Rust 2021). It imports files/folders into a knowledge-base vault via PowerShell or Windows Explorer right-click context menu, with SQLite-backed job history, SHA-256 deduplication, and toast notifications.

- Current branch: `v2.0` (template system, routing v2, Watch Mode, Win11 native context menu, system tray)
- Current release: `v1.0.1`

## Build, Test, and Development Commands

All Rust commands run from `kbintake/`:

```powershell
cargo build --release --locked --bins     # Release build
cargo test --locked                        # Unit + integration tests
cargo fmt --all -- --check                 # Format check
cargo clippy --all-targets --all-features --locked -- -D warnings  # Lint
cargo build --locked                       # Debug build
```

Installer build (from repo root, requires NSIS):
```powershell
New-Item -ItemType Directory -Force .\dist | Out-Null
Copy-Item .\kbintake\target\release\kbintake.exe .\dist\kbintake.exe -Force
Copy-Item .\kbintake\target\release\kbintakew.exe .\dist\kbintakew.exe -Force
Copy-Item .\kbintake\assets\kbintake.ico .\dist\kbintake.ico -Force
& "C:\Program Files (x86)\NSIS\makensis.exe" .\installer\kbintake.nsi
```

CI gates (GitHub Actions): formatting, clippy, build, test on `windows-latest`.

## Code Architecture

### Module Layout (`kbintake/src/`)

| Module | Responsibility |
|--------|---------------|
| `main.rs` | CLI entry point, command dispatch, exit code classification |
| `lib.rs` | Re-exports all public modules |
| `app.rs` | App bootstrap: config load, DB init, connection factory |
| `cli/mod.rs` | All CLI command handlers (import, jobs, targets, config, vault, explorer, doctor) |
| `config/mod.rs` | `AppConfig` (TOML), target management, v1 `routing`, v2 `routing_rules`, `templates` |
| `db/mod.rs` | DB connection wrapper, migration dispatcher |
| `db/schema.rs` | SQL migrations (001-core, 002-manifest+events, 003-event-index, 004-stored-sha256, 005-cli-tags, 006-import-subfolder) |
| `domain/` | Pure data types: `BatchJob`, `ItemJob`, `ManifestRecord`, `DomainEvent`, `Target` |
| `queue/` | SQLite repository (`repository.rs`) + state machine constants (`state_machine.rs`) |
| `processor/` | `scanner.rs` (walkdir), `hasher.rs` (SHA-256), `deduper.rs`, `copier.rs`, `validator.rs`, `frontmatter.rs`, `template.rs`, `dry_run.rs`, `audit.rs` |
| `agent/` | Background worker: `worker.rs` (item processing pipeline), `scheduler.rs` (queue drain), `watcher.rs` (file system watcher with directory preservation) |
| `adapter/local_folder.rs` | Storage adapter: copy to vault with conflict-safe naming, exact-path copy |
| `explorer/` | Windows registry ops for context menu install/uninstall, COM feasibility probe |
| `tray/` | System tray icon (`mod.rs`): Shell_NotifyIconW, right-click menu, settings launcher; autostart (`autostart.rs`): HKCU\Run management |
| `service.rs` | Windows Service lifecycle (install/start/stop/uninstall/dispatcher) |
| `notify.rs` | Windows toast notifications |
| `exit_codes.rs` | Structured exit codes (0=success, 1=general, 2=args, 3=size, 4=target, 5=reject, 6=partial, 7=dup, 8=db) |

### Processing Pipeline (per item)

1. **Validate** file existence + size → `validator.rs`
2. **Hash** source (SHA-256) → `hasher.rs`
3. **Resolve template** (v2 routing rules → `TemplateConfig` → `ResolvedTemplate`) → `template.rs`
4. **Deduplicate** against manifest by target+hash (verifies stored file exists) → `deduper.rs`
5. **Copy** to vault: if `import_subfolder` set → preserve directory structure with original name; else → template subfolder > target default_subfolder > target root with conflict-safe naming → `adapter/local_folder.rs`
6. **Frontmatter inject** for markdown → `frontmatter.rs`
7. **Hash stored file** → `hasher.rs`
8. **Record manifest** → `queue/repository.rs`
9. **Mark success/duplicate/failed** → state machine

### Config System

`%LOCALAPPDATA%\kbintake\config.toml` contains:
- `[[targets]]` - vault destinations with optional `default_subfolder`, `obsidian_vault`
- `[import]` - `max_file_size_mb`, `inject_frontmatter`, `language`, `auto_open_obsidian`
- `[agent]` - `poll_interval_secs`, `watch_in_service`
- `[[routing]]` - v1 extension-based rules (still supported)
- `[[templates]]` - v2 template definitions with `base_template` inheritance, `subfolder`, `tags`, `frontmatter`
- `[[routing_rules]]` - v2 multi-condition rules (extension, source_folder, file_name_contains, file_size range)
- `[[watch]]` - watch directory config with target, template, extensions, debounce

### Item State Machine

`queued` → `running` → `success` | `failed` | `duplicate`
Terminal states support: `undone`, `undo_skipped_modified`, `partially_undone`

## V2 Planning Alignment

For v2.0 work on branch `v2.0`, do NOT treat local implementation order as source of truth:

- **Product scope**: GitHub issue `#53` and `docs/PRD.md`
- **Phase tracking**: GitHub issues `#54` (Phase 1), `#55` (Phase 2), `#56` (Phase 3)
- **Repo-local tracking**: `docs/V2_ISSUE_MAP.md`

Key v2 epics and status:
- `#57` Windows 11 native context menu — COM DLL validated on hardware (open — go/no-go pending)
- `#58` Import template system — complete (closed)
- `#59` Target `default_subfolder` — complete (closed)
- `#60` TUI settings — complete (closed)
- `#61` zh-CN localization — complete (closed)
- `#62` Watch Mode — complete (closed)
- `#63` Obsidian URI integration — complete (closed)
- `#64` Quick tag injection — complete (closed)
- `#65` Vault audit — complete (closed)
- `#66` Clipboard import — complete (closed)
- `#76` System tray icon — complete (closed)
- `#77` Watch Mode directory structure preservation — complete (closed)

Known issue number conflicts: Epic `#58` references child tasks `#64`-`#69` which conflict with live tracker epics. Use Epic acceptance criteria, not inline child refs.

Before starting v2 work: identify governing issue, check for stale child refs, use Epic acceptance criteria. Update `docs/V2_ISSUE_MAP.md` after implementation.

## Key Files for Common Changes

- **New CLI subcommand**: `cli/mod.rs` (add enum variant + handler), `main.rs` (dispatch)
- **New processor stage**: `processor/` (new file + mod.rs export), wire into `agent/worker.rs`
- **Config change**: `config/mod.rs` (struct field + serde + validation), update `docs/CONFIGURATION.md`
- **DB schema change**: `db/schema.rs` (new migration), `db/mod.rs` (bump version + dispatch)
- **State machine change**: `queue/state_machine.rs` (new constant), `queue/repository.rs` (transition methods)

## Testing

- Unit tests live alongside modules (`#[cfg(test)] mod tests`)
- Integration tests in `kbintake/tests/` (currently `mvp_flow.rs`)
- Test naming: describe behavior, e.g., `rejects_missing_input_paths`
- Use `tempfile` for isolated test directories
