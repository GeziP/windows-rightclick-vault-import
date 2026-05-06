# KBIntake Project Status

Last updated: 2026-05-06

## Summary

KBIntake v2.1.1 is the latest release on branch `v2.0`. All planned features across Phase 1–3 are implemented, plus post-handoff additions (system tray, directory structure preservation, stale manifest handling). v2.1.1 fixed `--tags` frontmatter injection and added tray auto-start to the installer.

Current release:

```text
https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v2.1.1
```

## What Is Complete

### v1.0.0 Core (released)

- Rust CLI and Windows GUI-subsystem companion binary
- local config bootstrap under `%LOCALAPPDATA%\kbintake`
- SQLite schema migrations through schema version 6
- import queue, items, manifests, and events
- file scanning for files and directories
- validation, size limit checks, SHA-256 hashing
- local-folder target adapter
- deterministic copy conflict resolution
- duplicate detection per target
- Markdown frontmatter injection
- dry-run preview
- job list/show/retry/undo
- multiple target management
- extension-based routing rules
- vault stats command
- Explorer context-menu install/uninstall
- no-console Explorer import path via `kbintakew.exe`
- Windows toast notifications
- Windows Service lifecycle commands
- CI workflow for fmt, clippy, build, and tests
- release workflow for `v*.*.*` tags
- NSIS installer release asset
- repo-local winget manifest copy
- English README, Chinese README, install guide, configuration reference

### v2.0.0 Features (branch `v2.0`)

#### Epic #58: Import Template System

- `[[templates]]` and `[[routing_rules]]` config sections
- v1 `[[routing]]` compatibility retained
- template resolution with single-level `base_template` inheritance
- frontmatter merge/override
- tag merge/dedupe (case-insensitive)
- variable interpolation for 9 built-in variables
- conditional rendering (`{{#if}}` / `{{#else}}`)
- dry-run template preview
- `--template` / `-t` CLI flag for manual override
- `routing_rules.target` wired into actual import and dry-run
- route-hit visibility in previews, CLI output, and toast notifications

#### Epic #59: Target Default Subfolder

- `default_subfolder` config field on targets
- priority chain: template subfolder > target default_subfolder > target root

#### Epic #60: TUI Settings

- `kbintake tui` interactive terminal settings
- Tabbed layout: Targets, Import, Watch, Templates
- Keyboard shortcuts for all management operations
- Text input overlay for adding targets and watch paths
- Edit mode for `obsidian_vault` and watch config fields
- All labels localized

#### Epic #61: zh-CN Localization

- `[import].language` config option (`"en"` or `"zh-CN"`)
- All CLI output, toast notifications, and error messages translated
- TUI labels translated
- Explorer right-click menu text localized (e.g. "添加到知识库" for zh-CN)

#### Epic #62: Watch Mode

- `kbintake watch` CLI command
- `[[watch]]` config section for persistent watch paths
- `notify` crate for OS-level file events
- Debounce, extension filter, template binding per watch config
- Locked-file retry with backoff
- Windows Service integration via `agent.watch_in_service`
- Directory structure preservation: files imported into matching subdirectories with original names
- Startup scan: imports existing files not yet tracked in manifest
- Stale manifest detection: re-imports when vault file was deleted

#### Epic #63: Obsidian URI Integration

- `kbintake obsidian open --vault <name> <path>` command
- Per-target `obsidian_vault` config field
- `[import].auto_open_obsidian` global flag
- `--open` CLI flag for per-import override
- Auto-open after successful markdown import

#### Epic #64: Quick Tag Injection

- `--tags "urgent,alpha"` CLI flag
- Tags merged with template tags (case-insensitive dedup)
- DB migration 005: `cli_tags` column

#### Epic #65: Vault Audit

- `kbintake vault audit [--target] [--fix] [--json]` command
- Detects: orphan files, missing files, duplicate records, malformed frontmatter
- `--fix` cleans manifest records without deleting vault files
- `--json` structured output

#### Epic #66: Clipboard Import

- `--clipboard` CLI flag reads file paths from Windows clipboard
- Win32 `DataExchange` API for clipboard text access
- Combinable with `--tags`, `--process`, `--dry-run`

#### Epic #57: Windows 11 Native Context Menu

- COM DLL crate (`kbintake-com/`) with `IExplorerCommand` implementation
- Top-level context menu registration (not under "Show more options")
- Icon support via `GetIcon` returning `kbintake.ico` path
- GHA validation workflow for registry operations
- Validated on Windows 11 physical hardware

#### Default Templates (v2.1.1)

- 5 built-in templates (inbox, notes, documents, media, code) generated on first run
- 4 routing rules matching common file extensions to templates
- `inbox` as fallback for unmatched files
- Existing configs untouched; reset by deleting `config.toml`

#### Post-handoff Additions (no epic number)

- **System Tray Icon** (`kbintakew.exe tray`):
  - Shell_NotifyIconW with right-click context menu (Settings, Auto-start, Exit)
  - `src/tray/mod.rs`: hidden window + message loop + WndProc
  - `src/tray/autostart.rs`: HKCU\Run registry management for login persistence
  - Settings launches TUI via ShellExecuteW (avoids antivirus alerts)
  - File-based logging to `%LOCALAPPDATA%\kbintake\logs\`
- **Watch Directory Structure Preservation**:
  - DB migration 006: `items.import_subfolder` column
  - Files imported with original names into matching subdirectories
  - `LocalFolderAdapter::store_copy_to()` for exact-path copy
- **Stale Manifest Handling**:
  - Dedup verifies stored file still exists before marking duplicate
  - Deleted vault files trigger automatic re-import

## Validation State

```powershell
cargo test --locked                            # 171 tests (119 unit + 52 integration)
cargo clippy --all-targets --all-features -- -D warnings  # clean
cargo fmt --all -- --check                     # clean
cargo build --release --locked --bins          # kbintake + kbintakew
```

## Release History

- `v2.1.1` (2026-05-03) — Fix `--tags` frontmatter injection, tray autostart in installer, winget 2.0.0 manifest, default templates for first-run experience
- `v2.0.0` (2026-04-30) — Templates, Watch Mode, TUI, localization, Win11 context menu, system tray
- `v1.0.1` — Static CRT, installer validation workflow
- `v1.0.0` — Initial release

## Open Work

### Windows 11 Physical Validation (#57)

- COM DLL validated on physical hardware: install, top-level menu, icon, import, uninstall
- Remaining: go/no-go decision

### Documentation (#67)

- Configuration reference: complete (`docs/CONFIGURATION.md`)
- Template gallery: complete (`docs/TEMPLATE_GALLERY.md`)
- CONTRIBUTING.md: complete

### Winget Publication (#43)

- v2.1.1 PR submitted: `https://github.com/microsoft/winget-pkgs/pull/369491`
- Pending merge

### Distribution Hardening

- Authenticode signing for release binaries
- SmartScreen guidance
