# KBIntake Project Status

Last updated: 2026-04-28

## Summary

KBIntake v2.0 is in final development on branch `v2.0`. All planned features across Phase 1–3 are implemented. Remaining work is documentation, physical machine validation of the Windows 11 COM DLL, and release preparation.

The current stable release remains `v1.0.0`:

```text
https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0
```

## What Is Complete

### v1.0.0 Core (released)

- Rust CLI and Windows GUI-subsystem companion binary
- local config bootstrap under `%LOCALAPPDATA%\kbintake`
- SQLite schema migrations through schema version 5
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

#### Epic #62: Watch Mode

- `kbintake watch` CLI command
- `[[watch]]` config section for persistent watch paths
- `notify` crate for OS-level file events
- Debounce, extension filter, template binding per watch config
- Locked-file retry with backoff
- Windows Service integration via `agent.watch_in_service`

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

## Validation State

```powershell
cargo test --locked                            # 169 tests (117 unit + 52 integration)
cargo clippy --all-targets --all-features -- -D warnings  # clean
cargo fmt --all -- --check                     # clean
cargo build --release --locked --bins          # kbintake + kbintake-com
```

## Open Work

### Windows 11 Physical Validation (#57)

- COM DLL validated on physical hardware: install, top-level menu, icon, import, uninstall
- Remaining: go/no-go decision for v2.0 vs v2.1

### Documentation (#67)

- Configuration reference updated for v2.0
- Template gallery docs pending (#69, #73)
- Config.toml reference docs pending (#68, #72)
- CONTRIBUTING.md pending (#70)
- Release checklist and CHANGELOG format (#71)

### Release Preparation (#56)

- Version bump to `2.0.0`
- Installer update for new assets
- Winget manifest update for `2.0.0`
- Release notes

### Winget Publication (#43)

- PR submitted: `https://github.com/microsoft/winget-pkgs/pull/364698`
- Pending merge

### Distribution Hardening

- Authenticode signing for release binaries
- SmartScreen guidance
