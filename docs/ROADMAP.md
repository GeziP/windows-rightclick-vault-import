# Development Roadmap

This roadmap tracks KBIntake after the `v1.0.0` Windows release.

## Current Release

`v1.0.0` is available from GitHub Releases:

```text
https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0
```

The release includes:

- `KBIntake-Setup.exe`
- `kbintake.exe`
- `kbintakew.exe`
- `kbintake.ico`
- `SHA256SUMS.txt`

## v2.0 Status

All planned v2.0 features are implemented on branch `v2.0`:

| Epic | Feature | Status |
|------|---------|--------|
| #57 | Windows 11 native context menu | COM DLL validated on hardware |
| #58 | Import template system | Complete |
| #59 | Target default subfolder | Complete |
| #60 | TUI settings | Complete |
| #61 | zh-CN localization | Complete |
| #62 | Watch Mode | Complete |
| #63 | Obsidian URI integration | Complete |
| #64 | Quick tag injection | Complete |
| #65 | Vault audit | Complete |
| #66 | Clipboard import | Complete |

Remaining for v2.0 release:

- Documentation pass (#67)
- Windows 11 COM DLL go/no-go decision
- Installer update and version bump
- Winget manifest for `2.0.0`

## Post-handoff Additions

| Feature | Description |
|---------|-------------|
| System tray icon | `kbintakew.exe tray` with right-click menu (Settings, Auto-start, Exit) |
| Auto-start management | Toggle HKCU\Run registry entry from tray menu |
| Watch directory preservation | Files imported with original names into matching subdirectories (migration 006) |
| Stale manifest re-import | Dedup checks stored file existence; deleted vault files auto re-imported |
| Watch startup scan | Existing files in watch directories imported on watcher start |
| File-based logging | Tray and service modes log to `%LOCALAPPDATA%\kbintake\logs\` |

## Completed Milestones

### Foundation

- Rust crate scaffold and module boundaries
- CLI bootstrap, logging, config bootstrap, and SQLite initialization
- Windows CI for formatting, clippy, build, and tests

### Core Import Flow

- file and directory scanning
- SQLite-backed batches and items
- `kbintake import`
- `kbintake jobs list/show`
- validation, hashing, dedupe, deterministic copy conflict handling
- one-shot processing through `import --process`
- retry for failed jobs
- hash-safe undo

### Vault Management

- default target configuration
- multiple target add/list/show/rename/remove/set-default
- archived targets
- extension-based routing rules
- per-target vault stats
- vault audit (orphan/missing/duplicate/malformed detection)

### Templates and Routing

- import template system with variable interpolation and conditional rendering
- single-level template inheritance
- v2 multi-condition routing rules with template binding
- v1 routing compatibility
- per-target default subfolder
- quick tag injection (`--tags`)
- clipboard import (`--clipboard`)

### Usability

- TUI interactive settings (`kbintake tui`)
- zh-CN localization
- Obsidian URI integration with auto-open
- Watch Mode with debounce, extension filter, and template binding
- Watch Mode directory structure preservation (files keep original names and subdirectory paths)
- System tray icon with right-click context menu (Settings, Auto-start, Exit)
- Auto-start management via HKCU\Run registry
- File-based logging for tray and service modes

### Explorer And Notifications

- `kbintake explorer install/uninstall`
- Windows 11 native top-level context menu via COM DLL
- right-click menu registration for files and folders
- `kbintakew.exe` Windows-subsystem binary for no-console Explorer imports
- success, duplicate, and failure toast notifications
- manual validation through `scripts/validate-explorer-toast.ps1`

### Installer And Release

- NSIS per-user installer
- PATH update and uninstall entry
- Explorer registration during install
- release workflow triggered by `v*.*.*` tags
- `v1.0.0` GitHub Release assets
- repo-local winget manifest copy

### Background Processing

- `kbintake service install/start/stop/uninstall/status`
- hidden `service run` dispatcher
- service logging under `%LOCALAPPDATA%\kbintake\logs`
- queue processing from Windows Service mode
- Watch Mode integration with service mode
- manual SCM validation through `scripts/validate-service-mode.ps1`

## Planned Work

- v2.0 release: installer update, version bump, release notes
- Windows 11 COM DLL go/no-go for v2.0
- Documentation pass (template gallery, config reference, CONTRIBUTING)
- winget publication through `winget install GeziP.KBIntake` after PR merge
- Authenticode code signing
- installer option to install/start the Windows Service and tray
- reboot-resume validation for service mode

## Known Limitations

- release binaries are not code-signed, so Windows SmartScreen may warn
- winget package PR is submitted but not merged yet
- service install/start requires Administrator PowerShell
- service reboot-resume is not yet manually validated
- only local-folder vault targets are implemented
- TUI watch config editing is basic (cycles from first entry)
- COM DLL requires admin for HKCR registration

## Validation Commands

Run from `kbintake/`:

```powershell
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked
cargo build --release --locked --bins
```

Run from the repository root:

```powershell
.\scripts\validate-explorer-toast.ps1
.\scripts\validate-service-mode.ps1
```

`validate-service-mode.ps1` requires an elevated Administrator PowerShell session.
