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

### Explorer And Notifications

- `kbintake explorer install/uninstall`
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
- manual SCM validation through `scripts/validate-service-mode.ps1`

## Active Work

### Issue #43: winget publication

Status:

- manifest files exist under `installer/winget/1.0.0`
- installer URL points at the `v1.0.0` GitHub Release
- installer SHA-256 matches the release asset
- `winget validate --manifest .\installer\winget\1.0.0` passes
- PR submitted: `https://github.com/microsoft/winget-pkgs/pull/364698`

Remaining:

- monitor automated validation in `microsoft/winget-pkgs`
- run public install smoke after merge

### Epic #40: v1.0 distribution and polish

Mostly complete. It remains open because #43 is still open.

### Epic #45: v1.x background service

Service mode is implemented and validated, but the broader epic remains open for follow-up background-operation polish.

## Planned Features

- public winget installation through `winget install GeziP.KBIntake` after the package PR is merged
- Authenticode code signing
- installer option to install/start the Windows Service
- reboot-resume validation for service mode
- richer configuration editing commands for routing rules
- improved release notes and checksum verification guidance
- GitHub Actions dependency updates ahead of Node 20 deprecation
- future migration tests for any new schema changes

## Known Limitations

- release binaries are not code-signed, so Windows SmartScreen may warn
- winget package PR is submitted but not merged yet
- service install/start requires Administrator PowerShell
- service reboot-resume is not yet manually validated
- only local-folder vault targets are implemented

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
