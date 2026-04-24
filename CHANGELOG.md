# Changelog

## v1.0.0

Initial v1.0 release candidate for KBIntake, a Windows-friendly local vault import CLI.

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
- Release workflow that publishes `kbintake.exe`, `kbintake.ico`, and `SHA256SUMS.txt` for tagged releases.
- NSIS per-user installer that installs KBIntake, registers Explorer context menus, adds KBIntake to the user PATH, and provides an uninstaller.

### Notes

- The release binary is not Authenticode signed yet. Windows SmartScreen may warn on first run.
- Verify the downloaded executable manually:

```powershell
certutil -hashfile .\kbintake.exe SHA256
Get-Content .\SHA256SUMS.txt
```

The two SHA-256 values should match.
