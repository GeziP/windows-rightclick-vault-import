# KBIntake v2.0 Issue Map

Last updated: 2026-04-28

## Purpose

This file is the repo-local memory for v2.0 planning alignment.

Use it to keep implementation tied to the GitHub PRD and issue trackers:

- PRD: `#53`
- Phase 1 tracker: `#54`
- Phase 2 tracker: `#55`
- Phase 3 tracker: `#56`

Do not continue v2.0 implementation from local momentum alone. Before each new slice, map the work to a specific open issue or a clearly documented acceptance slice here.

## Normalized Source Of Truth

### Product / phase level

- `#53` PRD: KBIntake v2.0 product requirements
- `#54` Phase 1 tracker
- `#55` Phase 2 tracker
- `#56` Phase 3 tracker

### Core v2 epics

- `#57` Windows 11 native context menu
- `#58` Import template system
- `#59` Target `default_subfolder`
- `#60` TUI settings
- `#61` zh-CN localization
- `#62` Watch Mode
- `#63` Obsidian URI integration
- `#64` Quick tag injection
- `#65` Vault audit
- `#66` Clipboard import and release prep
- `#67` Documentation tracker

## Known Issue Number Conflicts

The Epic bodies are not fully trustworthy as child-task references.

Examples:

- Issue `#58` uses `#64`-`#69` as template-system child tasks.
- In the live tracker set, `#64`-`#66` are Phase 3 epics and `#67`-`#73` are documentation issues.

Implication:

- Do not assume inline child issue numbers inside `#58` or nearby epics are still valid.
- When in doubt, align to the Epic acceptance criteria, the PRD, and this file.

## Current Implementation Status

### Phase 1 / Epic `#58` Import template system

Implemented on branch `v2.0`:

- v2 config sections for `templates` and `routing_rules`
- config semantic validation
- v1 `routing` compatibility retained
- template resolution with single-level `base_template`
- frontmatter merge/override
- tag merge/dedupe
- variable interpolation for 9 built-in variables
- minimal conditional rendering with `if` / `else`
- dry-run template preview
- template application during actual import
- `routing_rules.target` wired into actual import and dry-run
- route-hit visibility in dry-run, CLI output, and Explorer toast copy using the matched template name as the current rule label

Covered by tests:

- config parsing and validation
- template rendering
- dry-run preview
- end-to-end routed import into target vault/subfolder

Still open for `#58`:

- any explicit Chinese-language error/output requirements not yet implemented

### Phase 1 / Epic `#62` Watch Mode

Implemented:

- `kbintake watch --path <dir>` command for directory watching
- `[[watch]]` config section for persistent watch paths
- Uses `notify` crate for OS-level file events
- Debounce layer prevents processing files still being written
- Extension filter and template binding per watch config
- Locked-file retry with backoff (3 attempts, 1s intervals)
- Reuses `resolve_import_intent()` for routing/template engine
- Queues files into existing SQLite import pipeline
- Windows Service integration: `agent.watch_in_service` config flag spawns
  watcher thread alongside queue processor on service start
- `Arc<AtomicBool>` shutdown flag for graceful service stop

Still open for `#62`:

(None)

### Phase 1 / Epic `#60` TUI settings

Implemented:

- `kbintake tui` — interactive terminal settings interface
- Tabbed layout: Targets, Import settings, Watch configs, Templates
- Keyboard shortcuts: q/Esc quit, 1-4 switch tabs, s save, a/r/d target management
- Frontmatter toggle, language toggle, max file size adjust
- All labels localized via `tr()`
- `ratatui` + `crossterm` for cross-platform terminal rendering

Still open for `#60`:

- Editing existing watch configs by row index (currently cycles from first entry)
- More advanced editing of template frontmatter

### Phase 1 / Epic `#63` Obsidian URI integration

Implemented:

- `kbintake obsidian open --vault <name> <note_path>` — opens note in Obsidian app
- Cross-platform URI launch (cmd /start on Windows, xdg-open on Linux)
- URL-encoded vault and file parameters
- `urlencoding` crate for proper URI escaping
- Per-target `obsidian_vault` config field
- Global `[import].auto_open_obsidian` boolean flag
- `--open` CLI flag for explicit per-import override
- Auto-opens markdown notes after successful import (worker.rs + CLI handler)

Still open for `#63`:

(None — field exists and is editable via config.toml or TUI 'e' key on Targets tab)

### Phase 1 / Epic `#61` zh-CN localization

Implemented:

- `kbintake/src/i18n.rs` — minimal `tr()` translation function
- `[import]` config section gains `language = "zh-CN"` option
- All CLI output messages localized (import, jobs, targets, vault, doctor)
- Toast notification messages localized (success, queued, failure)
- Dry-run table header localized
- Error messages in config, processor, explorer, service modules localized
- Defaults to `"en"` when `language` is unset

Still open for `#61`:

- any community-contributed refinements to Chinese wording

### Phase 1 / Epic `#58` — manual template override

Implemented:

- `--template` / `-t` flag on `kbintake import` CLI command
- `--template` flag on `explorer run-import` (hidden Explorer command)
- `AppConfig::resolve_import_intent()` consolidates routing logic with explicit overrides
- dry-run preview honours `--template` override
- Explorer right-click can now specify template via registry command args

### Phase 1 / Epic `#59` Target `default_subfolder`

Implemented on branch `v2.0`:

- config field on targets
- semantic validation for non-empty relative paths
- priority chain:
  - template `subfolder`
  - target `default_subfolder`
  - target root
- actual import writes to computed subfolder
- dry-run preview reflects computed subfolder

Still open for `#59`:

- confirm `doctor` surfaces target subfolder validation in the intended UX
- confirm any missing CLI/TUI editing flows

### Phase 1 / Epic `#57` Windows 11 native context menu

COM DLL proof of concept completed.

Implemented on branch `v2.0`:

- hidden probe command: `kbintake explorer com-feasibility`
- repo-local spike report: `docs/WIN11_COM_FEASIBILITY.md`
- initial architecture verdict: proceed only with a separate DLL-oriented spike
- separate COM DLL crate (`kbintake-com/`):
  - manual vtable `IExplorerCommand` implementation
  - `IClassFactory` for COM instantiation
  - `DllMain`, `DllGetClassObject`, `DllCanUnloadNow` exports
  - HKCR registration/unregistration binary
  - `Invoke` spawns `kbintake.exe import --process` in background

Automated validation:

- `.github/workflows/com-validation.yml` — GHA workflow: build DLL, reg install, verify HKCR keys, reg uninstall, verify cleanup
- runs on push/PR touching `kbintake-com/` and on manual dispatch

Still required by Phase 1 tracker:

- real Windows 11 Explorer UI test on a physical machine (right-click visibility, "Show more options", toast, end-to-end import)
- go/no-go decision for v2.0 vs v2.1

### Phase 2/3 / Epic `#64` Quick tag injection

Implemented:

- `--tags "urgent,alpha"` CLI flag on `kbintake import` and `explorer run-import`
- DB migration 005: `items.cli_tags TEXT` column
- `ItemJob.cli_tags` field, persisted through repository
- `render_template()` merges CLI tags with template tags (case-insensitive dedup)
- Tags merged into frontmatter via existing template_frontmatter path
- Dry-run preview includes CLI tags
- `--tags` on Explorer right-click via registry command args

### Phase 2/3 / Epic `#65` Vault audit

Implemented:

- `kbintake vault audit [--target <name>] [--fix] [--json]` command
- `processor/audit.rs`: scans vault directory with walkdir, cross-references manifest records
- Detects 4 issue types: orphan files, missing files, duplicate SHA-256 records, malformed frontmatter
- `--fix` auto-cleans manifest records for deleted files and deduplicates records (never deletes vault files)
- `--json` outputs structured `Vec<AuditReport>`
- i18n support for en/zh-CN audit messages
- Repository methods: `list_manifests_by_target`, `mark_manifest_missing`, `mark_manifest_duplicate`

### Phase 2/3 / Epic `#66` Clipboard import

Implemented:

- `kbintake import --clipboard` flag
- `clipboard.rs`: reads Windows clipboard text via `Win32::System::DataExchange` API
- Splits clipboard text by newlines, validates each line as existing file path
- Merges clipboard paths with explicit `paths` arguments
- Combinable with `--tags`, `--process`, `--dry-run`
- `Win32_System_DataExchange` feature added to `windows` crate dependency

## Working Rule For Future v2 Slices

Before coding:

1. Identify the governing issue.
2. Check whether the issue body contains stale child references.
3. Use the acceptance criteria in the Epic and PRD as the binding target.
4. Update this file after any meaningful slice lands.

## Recommended Next Slice

All planned v2.0 features implemented. Remaining:

- Real Windows 11 validation of the `kbintake-com` DLL on a physical machine (`#57`)
- Documentation pass (`#67`)
- Release preparation (version bump, CHANGELOG, installer update)
