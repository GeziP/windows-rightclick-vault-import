# KBIntake v2.0 Handoff Notes

Last updated: 2026-04-28

## Purpose

This file is the current implementation and planning handoff for continuing KBIntake v2.0 development in another agent session.

Use this together with:

- `docs/PRD.md`
- `docs/V2_DEVELOPMENT_PLAN.md`
- `docs/V2_ISSUE_MAP.md`
- GitHub issues `#53`–`#67`

## Current Branch State

- active branch: `v2.0`
- working tree status at handoff: clean
- all Phase 1–3 features implemented

Recent v2 commits on this branch:

- `f4e903e` Add --tags, vault audit, and --clipboard features (#64, #65, #66)
- `1653ef6` Win11 native top-level context menu + COM icon + GHA validation
- `0f20293` Add TUI edit mode: target obsidian_vault and watch config fields
- `098f808` Add Service Watch Mode, TUI text input, and Obsidian auto-open (#60, #62, #63)
- `51453c6` Add TUI settings, Obsidian URI, and Watch Mode completion (#60, #62, #63)
- `5d41f48` Add manual template override for import (#58)
- `5a15f4d` Add kbintake-com COM DLL spike for Windows 11 native context menu
- `ea5203e` Apply templates during import
- `11c1e8e` Add template conditional rendering

## Source Of Truth

Do not continue from local code momentum alone.

For v2.0 work, use:

- product requirements: issue `#53` and `docs/PRD.md`
- phase tracking: issues `#54`, `#55`, `#56`
- normalized repo-local mapping: `docs/V2_ISSUE_MAP.md`

## What Is Implemented

### Phase 1 / `#58` Import template system

- `templates` and `routing_rules` config sections
- v1 `routing` compatibility retained
- template resolution with single-level `base_template` inheritance
- frontmatter merge/override, tag merge/dedupe
- 9 built-in interpolation variables
- conditional rendering (`{{#if}}` / `{{#else}}`)
- `--template` / `-t` CLI flag
- `routing_rules.target` wired into import and dry-run
- route-hit visibility in previews, CLI output, and toast notifications

### Phase 1 / `#59` Target `default_subfolder`

- per-target `default_subfolder` config field
- priority chain: template subfolder > target default_subfolder > target root

### Phase 1 / `#57` Windows 11 native context menu

- COM DLL crate (`kbintake-com/`) with `IExplorerCommand` implementation
- Top-level context menu (not under "Show more options")
- Icon support via `GetIcon`
- GHA validation workflow
- Validated on Windows 11 physical hardware

### Phase 1 / `#60` TUI settings

- `kbintake tui` interactive settings with tabs for Targets, Import, Watch, Templates
- Text input overlay, edit mode for `obsidian_vault` and watch config fields

### Phase 1 / `#61` zh-CN localization

- `[import].language` config option
- All CLI output, toast, and TUI labels translated

### Phase 1 / `#62` Watch Mode

- `kbintake watch` CLI command with `[[watch]]` config
- Debounce, extension filter, template binding, locked-file retry
- Service integration via `agent.watch_in_service`

### Phase 1 / `#63` Obsidian URI integration

- `kbintake obsidian open --vault <name> <path>`
- Per-target `obsidian_vault`, global `auto_open_obsidian`, `--open` CLI flag
- Auto-open after successful markdown import

### Phase 2/3 / `#64` Quick tag injection

- `--tags "urgent,alpha"` CLI flag
- DB migration 005: `cli_tags` column
- Tags merged with template tags (case-insensitive dedup)

### Phase 2/3 / `#65` Vault audit

- `kbintake vault audit [--target] [--fix] [--json]`
- Detects orphan, missing, duplicate, malformed frontmatter
- `--fix` cleans manifest without deleting vault files

### Phase 2/3 / `#66` Clipboard import

- `--clipboard` CLI flag reads file paths from Windows clipboard
- Win32 `DataExchange` API

## Recommended Next Step

All planned v2.0 features are implemented. Next steps:

1. **Documentation pass** (`#67`): template gallery, config reference, CONTRIBUTING
2. **Windows 11 COM DLL go/no-go**: decide if it ships in v2.0 or v2.1
3. **Release preparation** (`#56`): version bump, installer update, winget manifest, release notes

## Validation State At Handoff

```powershell
cargo test --locked                            # 169 tests (117 unit + 52 integration)
cargo clippy --all-targets --all-features -- -D warnings  # clean
cargo fmt --all -- --check                     # clean
cargo build --release --locked --bins          # kbintake + kbintake-com
```

## Files Most Relevant To Continue From

- `docs/V2_ISSUE_MAP.md`
- `docs/WIN11_COM_FEASIBILITY.md`
- `kbintake-com/src/command.rs` — IExplorerCommand vtable
- `kbintake-com/src/reg.rs` — HKCR registration helpers
- `kbintake/src/config/mod.rs` — ImportRoutingIntent + resolve_import_intent + WatchConfig + language
- `kbintake/src/cli/mod.rs` — all CLI handlers with --template, --tags, --clipboard, vault audit
- `kbintake/src/processor/audit.rs` — vault audit logic
- `kbintake/src/processor/template.rs` — template rendering with CLI tags merge
- `kbintake/src/clipboard.rs` — Windows clipboard reader
- `kbintake/src/agent/watcher.rs` — Watch Mode with PID lock + toast
- `kbintake/src/i18n.rs` — en/zh-CN translation dictionaries
- `kbintake/src/tui/mod.rs` — interactive TUI settings
- `kbintake/src/obsidian.rs` — Obsidian URI integration
- `kbintake/tests/mvp_flow.rs` — integration tests
