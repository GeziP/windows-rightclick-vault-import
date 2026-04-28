# Changelog

## v2.0.0 (Unreleased)

Major release adding template-based imports, Watch Mode, TUI settings, localization, and Windows 11 native context menu support.

### Added

- Import template system with variable interpolation, conditional rendering, and single-level inheritance (`[[templates]]`, `[[routing_rules]]`)
- Per-target `default_subfolder` configuration
- `kbintake config validate` command for config semantic checks
- `--template` / `-t` CLI flag for manual template override on import
- `--tags` CLI flag for quick tag injection (comma-separated, merged with template tags)
- `--clipboard` CLI flag to import file paths read from Windows clipboard
- Watch Mode: `kbintake watch` monitors directories for new files with debounce, extension filter, and template binding
- `[[watch]]` config section for persistent watch paths
- `agent.watch_in_service` config flag for Watch Mode in Windows Service
- `kbintake tui` interactive terminal settings with target management, import settings, watch configs, and templates tabs
- zh-CN localization via `[import].language = "zh-CN"` config option
- Explorer right-click menu text localized (e.g. "添加到知识库" for zh-CN, "Add to Knowledge Base" for en)
- `kbintake obsidian open --vault <name> <path>` command
- Per-target `obsidian_vault` config field and global `auto_open_obsidian` flag
- `--open` CLI flag to open imported notes in Obsidian after import
- `kbintake vault audit [--target] [--fix] [--json]` command detecting orphan, missing, duplicate, and malformed-frontmatter files
- `vault audit --fix` auto-cleans manifest records without deleting vault files
- DB schema migration 004: `stored_sha256` column for verified post-copy integrity
- DB schema migration 005: `cli_tags` column for tag persistence
- Windows 11 native context menu: COM DLL (`kbintake-com/`) with `IExplorerCommand` implementation
- GHA workflow for COM DLL registry validation
- Route-hit visibility in dry-run output, CLI output, and Explorer toast notifications

### Changed

- Dry-run output now shows target, matched routing rule, template destination, and frontmatter preview
- Explorer import toast text includes routing rule context
- SQLite schema version bumped to 5
- `windows` crate dependency updated with `Win32_System_DataExchange` feature for clipboard support

### Notes

- v1 `[[routing]]` config is still supported; v2 `[[routing_rules]]` take priority
- All 169 tests passing (117 unit + 52 integration)
- COM DLL validated on Windows 11 physical hardware (top-level context menu, icon, install/uninstall)

## v1.0.1

Patch release focused on Windows installer reliability and release verification.

### Changed

- Release builds now use a static CRT configuration so the shipped binaries do not depend on external VC++ runtime DLL installation.
- Added a GitHub Actions installer validation workflow that covers NSIS build, winget manifest validation, silent install, command smoke tests, and uninstall on Windows.

### Fixed

- Removed the need for a separate VC++ runtime dependency to avoid clean-machine startup failures around `VCRUNTIME140.dll`.

## v1.0.0

Initial v1.0 release for KBIntake, a Windows-friendly local vault import CLI and Explorer integration.

### Added

- Terminal import flow for files and directories.
- SQLite-backed batches, items, manifests, and audit events.
- Local vault target management with add, list, show, rename, remove, and set-default commands.
- Multiple target imports with explicit `--target` selection.
- One-shot processing with `import --process` and background queue draining with `agent`.
- SHA-256 hashing, deduplication, validation, deterministic copy conflict handling, and manifest recording.
- Dry-run previews with table and JSON output.
- Job inspection with list/show, JSON/table output, retry, and safe undo.
- Hash-safe undo behavior that protects modified destination files.
- Markdown frontmatter injection with `[import].inject_frontmatter` opt-out.
- Per-target vault stats with JSON output.
- Explorer context-menu install/uninstall commands and reviewable registry scripts.
- Manual validation scripts for Explorer toast/no-console behavior and Windows Service mode.
- Windows CI for formatting, clippy, build, and test gates.
- Release workflow that publishes `KBIntake-Setup.exe`, `kbintake.exe`, `kbintakew.exe`, `kbintake.ico`, and `SHA256SUMS.txt` for tagged releases.
- NSIS per-user installer that installs KBIntake, registers Explorer context menus, adds KBIntake to the user PATH, and provides an uninstaller.

### Notes

- The release binaries and installer are not Authenticode signed yet. Windows SmartScreen may warn on first run.
- Verify the downloaded executable manually:

```powershell
certutil -hashfile .\kbintake.exe SHA256
Get-Content .\SHA256SUMS.txt
```

The two SHA-256 values should match.
