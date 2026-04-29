# Release Checklist

Use this checklist for KBIntake releases. Adapt the version numbers as needed.

## Version Numbering (SemVer)

| Type | Bump | When |
|------|------|------|
| **Patch** | `x.y.Z` | Bug fixes only |
| **Minor** | `x.Y.0` | New features, backward compatible |
| **Major** | `X.0.0` | Breaking changes |

## Pre-release Gates (all must pass before tagging)

- [ ] CI green on the release branch: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`
- [ ] All planned epics/issues for this version are closed
- [ ] No P0 bugs open
- [ ] Config reference docs in sync with code structs (`kbintake config validate` passes with doc examples)
- [ ] Template gallery examples parse correctly
- [ ] End-to-end validation on Windows 10 and Windows 11
- [ ] Release binary size: `kbintake.exe` < 15 MB
- [ ] Winget manifest updated with correct version and SHA256

## Automated Checks

Run from `kbintake/`:

```powershell
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --release --locked --bins
```

## Release Assets

Expected GitHub Release assets:

- `KBIntake-Setup.exe` (NSIS installer)
- `kbintake.exe`
- `kbintakew.exe`
- `kbintake.ico`
- `SHA256SUMS.txt`

## Smoke Tests

### Install & CLI

```powershell
.\dist\KBIntake-Setup.exe
kbintake --version
kbintake doctor
```

### Explorer Context Menu

```powershell
.\scripts\validate-explorer-toast.ps1
```

Verify:
- Right-click import works for files and folders
- Toast notifications appear (success, duplicate, failure)
- No console window for Explorer-triggered imports
- Cascading submenu shows Import / Queue / Settings

### Watch Mode

```powershell
kbintake watch --path <test-dir>
```

Verify:
- New files in watched directory auto-import
- Directory structure preserved (subdirectories kept, files not renamed)
- Startup scan imports existing files
- Duplicate detection works

### Service Mode (elevated PowerShell)

```powershell
.\scripts\validate-service-mode.ps1
```

### System Tray

```powershell
kbintakew.exe tray
```

Verify:
- Tray icon appears
- Right-click menu shows Settings / Auto-start / Exit
- Settings opens TUI
- Auto-start toggles HKCU\Run
- Exit cleans up

## Release Execution

1. Update `version` in `kbintake/Cargo.toml`
2. Update `CHANGELOG.md`: rename `[Unreleased]` to `[version] - date`, add new empty `[Unreleased]`
3. Final local CI: `cargo build --release && cargo test && cargo clippy && cargo fmt --check`
4. Commit: `git commit -m "chore: release v2.0.0"`
5. Tag: `git tag -a v2.0.0 -m "KBIntake v2.0.0"`
6. Push: `git push origin main && git push origin v2.0.0`
7. Verify GitHub Actions release workflow succeeds
8. Edit GitHub Release with release notes
9. Submit winget manifest PR to `microsoft/winget-pkgs`
10. Update documentation issues

## CHANGELOG Format

Follow [Keep a Changelog](https://keepachangelog.com). Use these sections (only include non-empty ones):

- `### Added` — new features
- `### Changed` — behavior changes
- `### Fixed` — bug fixes
- `### Deprecated` — upcoming removals
- `### Removed` — removed features
- `### Security` — security fixes

Entries should:
- Start with a verb: "Add", "Fix", "Change"
- Be user-facing, not implementation details
- Reference issue numbers: `Add import template system (#58)`

## Rollback

```powershell
kbintake explorer uninstall
kbintake service stop
kbintake service uninstall
& "$env:LOCALAPPDATA\Programs\kbintake\Uninstall.exe"
```

Runtime state lives under `%LOCALAPPDATA%\kbintake`. Do not delete unless intentionally clearing all data.

## Known Limitations

- Release binaries are not Authenticode signed (SmartScreen may warn)
- Winget availability pending `microsoft/winget-pkgs` PR merge
- Service reboot-resume needs manual validation
- Only local-folder targets supported
- TUI watch config editing is basic
- COM DLL requires admin for HKCR registration
