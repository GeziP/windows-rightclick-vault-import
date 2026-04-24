# KBIntake Project Status

Last updated: 2026-04-24

## Summary

KBIntake v1.0.0 is released as a Windows local vault import tool. The release includes a GitHub-hosted NSIS installer, Explorer right-click integration, toast notifications, queue-backed imports, deduplication, undo, vault stats, and optional Windows Service processing.

Release page:

```text
https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0
```

## What Is Complete

- Rust CLI and Windows GUI-subsystem companion binary
- local config bootstrap under `%LOCALAPPDATA%\kbintake`
- SQLite schema migrations through schema version 3
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

## Validation Completed

- `cargo fmt --all -- --check`
- `cargo test --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo build --locked`
- `cargo build --release --locked --bins`
- Explorer toast/no-console validation through `scripts/validate-explorer-toast.ps1`
- Windows Service install/start/process/log/stop/uninstall validation through `scripts/validate-service-mode.ps1`
- installer build through GitHub Actions release workflow
- release asset upload for `v1.0.0`
- winget manifest validation with `winget validate --manifest .\installer\winget\1.0.0`

## Open Work

### Winget publication

Issue: #43

Current status:

- release URL exists
- installer SHA-256 is recorded in `installer/winget/1.0.0`
- manifest validation passes

Remaining:

- enable local manifest installs on a test machine
- run `winget install --manifest .\installer\winget\1.0.0`
- submit a PR to `microsoft/winget-pkgs`
- link the PR in issue #43

### Distribution hardening

Planned:

- Authenticode signing for release binaries
- clearer SmartScreen guidance in release notes
- possible installer option to install/start the Windows Service

### Service validation

Implemented:

- service install/start/status/stop/uninstall
- background queue processing
- service log creation

Remaining:

- reboot-resume smoke validation
- optional installer integration for service setup

### Maintenance

Planned:

- keep GitHub Actions dependencies current
- update actions before Node 20 runner deprecation becomes blocking
- continue adding migration coverage before future schema changes

## Release Assets

The `v1.0.0` GitHub Release publishes:

- `KBIntake-Setup.exe`
- `kbintake.exe`
- `kbintakew.exe`
- `kbintake.ico`
- `SHA256SUMS.txt`

Users should prefer `KBIntake-Setup.exe`.

## Repository Notes

- `dist/` is intentionally ignored and used only for local release staging.
- runtime databases, logs, and machine-specific registry exports should not be committed.
- service validation requires an elevated Administrator PowerShell session.
