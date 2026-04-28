# KBIntake v2.0 Development Plan

Last updated: 2026-04-28

## Version Theme

KBIntake v2.0 moves the project from a working Windows import utility to a daily-use local intake system for Obsidian-oriented knowledge work.

The release should prioritize:

- template-based imports
- predictable routing into vault subfolders
- non-developer setup and Chinese-language usability
- passive folder watching
- safer long-term vault maintenance

Windows 11 native Explorer integration remains important, but it is the highest-risk item. It should run behind an early feasibility gate rather than block all other v2 work.

## Source Of Truth

Existing GitHub planning:

- PRD: `#53`
- Phase 1 tracker: `#54`
- Phase 2 tracker: `#55`
- Phase 3 tracker: `#56`
- Documentation tracker: `#67`
- Repo-local issue map: `docs/V2_ISSUE_MAP.md`

Important:

- Continue v2.0 implementation against GitHub issues, not only against this document.
- Some older issue references inside Epic bodies conflict with later documentation issue numbers.
- Use `docs/V2_ISSUE_MAP.md` as the normalized mapping before starting the next v2 slice.

Core v2 epics:

- `#57` Windows 11 native context menu
- `#58` import template system
- `#59` target default subfolder
- `#60` TUI settings
- `#61` zh-CN localization
- `#62` Watch Mode
- `#63` Obsidian URI integration
- `#64` quick tag injection
- `#65` vault audit
- `#66` clipboard import and release prep

## Scope Decision

### Must Ship

- v1.0 compatibility for existing `config.toml` and SQLite data
- `default_subfolder` for targets
- template definitions with frontmatter fields, tags, and subfolder rules
- variable interpolation for the documented nine built-in variables
- routing rules using the documented v2 field names
- CLI import support for template, subfolder, tags, clipboard, and JSON output
- config validation and migration tests
- zh-CN user-facing text for the new workflow
- Watch Mode without data loss under common file-write races
- updated README, install docs, config reference, and changelog

### Should Ship

- TUI setup wizard
- Obsidian URI open after import
- `vault audit`
- installer option for service/watch setup
- Windows 11 native Explorer command if the feasibility gate passes

### Can Slip To v2.1

- system tray management UI
- rich Windows toast action buttons
- automatic community template gallery integration
- advanced nested template inheritance
- any COM approach that requires fragile installer behavior

## Recommended Development Order

The GitHub trackers group work by user-facing phase. The implementation order should be slightly different so the riskiest and most shared contracts are handled early.

### Phase 0: Stabilize v1.0 And Prepare Branch

Target duration: 1-2 days

Tasks:

- finish winget PR follow-up for `v1.0.0`
- create a `v2.0` development branch
- run the full v1 validation suite before any v2 schema changes
- add a top-level `## [Unreleased]` changelog area for v2 work
- decide whether `v2.0.0` will include a database schema migration

Exit criteria:

- `cargo fmt --all -- --check`
- `cargo test --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo build --release --locked --bins`

### Phase 1: Configuration, Templates, Routing, Paths

Target duration: 1.5-2 weeks

This is the technical foundation. Start here before TUI, Watch Mode, and Explorer submenus.

Tasks:

- replace v1 `routing` config with v2-compatible `routing_rules` while preserving old config compatibility
- add `templates` and `default_subfolder` to config structs
- implement `kbintake config validate`
- implement template resolution:
  - single-level `base_template`
  - frontmatter merge and override
  - tag merge and dedupe
  - subfolder priority chain
- implement interpolation variables:
  - `file_name`
  - `file_ext`
  - `file_size_kb`
  - `imported_at`
  - `imported_at_date`
  - `source_path`
  - `sha256`
  - `target_name`
  - `batch_id`
- implement the minimal conditional syntax only after interpolation is well tested
- update local-folder adapter so it can store into computed subfolders
- add `--template`, `--subfolder`, and `--tags` to `kbintake import`
- update dry-run output to show target, template, subfolder, and destination

Exit criteria:

- existing v1 tests pass unchanged or with documented compatibility updates
- config examples from `#72` parse successfully
- template examples from `#73` either pass or have documented deviations
- imports with no v2 config behave like v1.0

### Phase 1 Risk Track: Windows 11 Native Menu Feasibility

Target duration: 3 days, parallel with Phase 1

This must be a spike, not a full implementation.

Tasks:

- create a minimal proof of concept for `IExplorerCommand`
- decide between Rust-only COM and a small C++ shim
- prove install and uninstall in a clean Windows 11 VM
- measure right-click menu latency
- document fallback behavior

Decision gate:

- If the POC can register, render one static command, invoke `kbintakew.exe`, and uninstall cleanly by day 3, continue toward v2.0.
- If not, keep the v1 registry menu and move native Windows 11 menu to v2.1.

### Phase 2: Usability And Automation

Target duration: 2 weeks

Tasks:

- implement zh-CN language selection:
  - config field
  - optional global CLI override
  - user-facing command output for new v2 flows
- implement `kbintake settings` as a focused TUI:
  - first-run setup
  - target management
  - template selection/editing
  - routing rule validation
- implement Watch Mode core:
  - folder watcher
  - debounce
  - extension filter
  - template and target binding
  - locked-file retry
  - duplicate watcher detection
- integrate Watch Mode with service mode where practical

Exit criteria:

- first-run setup can create a target, template, and routing rule without manual TOML editing
- Watch Mode imports a newly written file after the write completes
- locked-file retries are covered by tests or a scripted manual validation
- zh-CN docs match the shipped commands

### Phase 3: Workflow Integrations And Maintenance

Target duration: 1.5-2 weeks

Tasks:

- implement `kbintake import --clipboard`
- implement Obsidian URI construction and optional auto-open
- implement `vault audit`:
  - orphan files
  - missing manifest files
  - duplicate manifest records
  - malformed KBIntake frontmatter
- implement `vault audit --fix` only for safe metadata repairs
- add quick tag injection to CLI import
- add Explorer quick-tag entry only if the dialog approach is stable

Exit criteria:

- audit never deletes user vault files
- clipboard import handles single path and multi-line paths
- Obsidian integration degrades cleanly when Obsidian is absent
- all workflow features support JSON or scriptable output where applicable

### Phase 4: Release Hardening

Target duration: 1 week

Tasks:

- migration test from real-looking v1 config and database fixtures
- Windows 10 and Windows 11 end-to-end validation
- installer update for new assets and optional service/watch setup
- release binary size check
- README, README.zh-CN, install docs, config reference, template gallery, and changelog
- winget manifest update for `v2.0.0`

Exit criteria:

- no open P0 issues
- all planned v2 epics either closed or explicitly moved to v2.1
- release checklist completed
- GitHub Release draft includes rollback and upgrade notes

## First Development Slice

Start with a narrow PR that changes no import behavior:

1. Add v2 config structs for `templates`, `routing_rules`, and `default_subfolder`.
2. Preserve deserialization of the existing v1 `routing` field.
3. Add `kbintake config validate`.
4. Add tests for v1 config compatibility and v2 example parsing.
5. Update docs only for the fields actually implemented in the PR.

This slice creates the contract that later template rendering, Watch Mode, TUI, and Explorer integration can all depend on.

## Technical Notes

- Keep schema migrations append-only and covered by integration tests.
- Keep template rendering separate from file copying.
- Keep routing resolution pure and unit-tested before wiring it into import.
- Avoid making the Windows 11 COM module part of the main Rust binary until the installation model is proven.
- Treat TUI as a config editor over existing APIs, not a parallel implementation.
- Keep `kbintakew.exe` as the no-console invocation path for Explorer-triggered commands.

## Validation Matrix

Every feature PR should run:

```powershell
cd kbintake
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Release candidates should additionally run:

```powershell
cargo build --release --locked --bins
.\scripts\validate-explorer-toast.ps1
.\scripts\validate-service-mode.ps1
winget validate --manifest .\installer\winget\2.0.0
```

Manual release validation must cover:

- clean install
- upgrade from v1.0
- Explorer import
- Watch Mode import
- duplicate import
- undo after template frontmatter injection
- vault audit with and without `--fix`
- zh-CN output
- uninstall cleanup
