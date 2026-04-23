# Development Roadmap

This backlog turns the current Rust scaffold into an MVP for a Windows right-click vault import tool. The execution strategy is: make the CLI reliable first, add a background worker second, then integrate Explorer context menus.

## Epic E1: Rust Crate Foundation

Goal: create a compilable `kbintake` Rust application with predictable module boundaries.

- Issue I01: Scaffold Cargo project
  - Create `kbintake/Cargo.toml`, `src/main.rs`, and module directories from `kbintake_rust_scaffold.md`.
  - Acceptance: `cargo fmt` and `cargo build` run from `kbintake/`.
- Issue I02: Add logging and app bootstrap
  - Implement `logging::init_logging()` and `App::bootstrap()`.
  - Acceptance: CLI starts, initializes config and database paths, and emits structured logs.
- Issue I03: Normalize developer setup
  - Document Rust toolchain installation and Windows prerequisites.
  - Acceptance: README includes setup steps and common troubleshooting.

## Epic E2: Configuration and SQLite Storage

Goal: persist import jobs and configuration locally without requiring external services.

- Issue I04: Implement app config
  - Load or initialize config under the user data directory.
  - Acceptance: `kbintake config-show` prints effective target and import settings.
- Issue I05: Create SQLite schema
  - Add tables for batches, items, manifest records, and events.
  - Acceptance: `doctor` creates or validates the database idempotently.
- Issue I06: Implement queue repository
  - Add insert, lookup, list, status update, and manifest operations.
  - Acceptance: repository unit tests cover happy path and missing-row behavior.

## Epic E3: CLI Import Workflow

Goal: allow users to enqueue files or directories from a terminal.

- Issue I07: Implement scanner
  - Expand file and directory inputs into importable files.
  - Acceptance: nested directories are walked and missing inputs fail clearly.
- Issue I08: Implement `import`
  - Create a batch and item rows for each discovered file.
  - Acceptance: `kbintake import <path>` prints batch ID, item count, and target.
- Issue I09: Implement `jobs list/show`
  - Display recent batches and item details.
  - Acceptance: users can inspect queued, running, successful, failed, and duplicate items.

## Epic E4: Processing Pipeline and Agent

Goal: move queued work through validation, hashing, dedupe, copy, and manifest creation.

- Issue I10: Add validation and hashing
  - Check file existence, type, size limits, and compute SHA-256.
  - Acceptance: invalid files are marked failed with stable error codes.
- Issue I11: Add local-folder adapter
  - Copy source files into the configured vault target safely.
  - Acceptance: name conflicts are handled deterministically without overwriting content.
- Issue I12: Add dedupe and manifest writes
  - Detect duplicate target/hash pairs and record successful imports.
  - Acceptance: duplicate imports do not create extra file copies.
- Issue I13: Implement agent loop
  - Poll queued items, process them, and update item/batch status.
  - Acceptance: `kbintake agent` drains a queued batch end to end.

## Epic E5: Windows Explorer Integration

Goal: expose the import command through file and directory right-click menus.

- Issue I14: Generate registry scripts
  - Add file and directory context-menu registration scripts with placeholders.
  - Acceptance: scripts are reviewable and do not hard-code developer-local paths.
- Issue I15: Add installer guidance
  - Document how to build the exe, place it, and apply/remove registry entries.
  - Acceptance: README includes registration, unregistration, and safety notes.

## Epic E6: Quality, Release, and Hardening

Goal: make the MVP testable, diagnosable, and safe for local use.

- Issue I16: Add integration tests
  - Test config bootstrap, database migration, import enqueue, and worker success.
  - Acceptance: tests use temporary directories and do not touch real user vaults.
- Issue I17: Add CI workflow
  - Run format, clippy, build, and tests on pull requests.
  - Acceptance: GitHub Actions status reports pass/fail for every PR.
- Issue I18: Prepare v0.1 release checklist
  - Define manual Windows validation steps and known limitations.
  - Acceptance: release notes list commands tested, Windows version, and rollback steps.

## Planned Execution Order

1. E1 foundation.
2. E2 configuration and database.
3. E3 CLI enqueue and inspection.
4. E4 processing agent.
5. E6 tests and CI for the completed MVP path.
6. E5 Explorer integration after CLI and agent are stable.

## Current Status

- GitHub issue tracking is available; MVP processing loop work is tracked in issue #25.
- Rust build/test tooling is available in the current shell.
- The CLI import and one-shot agent path has focused unit coverage for scanning, queue state, deterministic copy conflicts, duplicate detection, invalid sources, and partial-import rejection.
- Integration tests cover config bootstrap, idempotent schema initialization, import enqueue, successful agent drain, and duplicate handling.
- CI workflow coverage is defined for Windows with formatting, clippy, build, and test gates.
- Explorer registration now includes unregister scripts plus README install, registration, verification, and rollback guidance.
- v0.1 release validation is tracked in `docs/RELEASE_CHECKLIST.md`.
- Target configuration can be updated from the CLI with `kbintake config set-target`, and `doctor` validates schema plus target writability.
- Multiple targets can be added/listed from the CLI, and imports can explicitly select a target with `--target`.
- Imports can optionally process immediately with `kbintake import --process`.
