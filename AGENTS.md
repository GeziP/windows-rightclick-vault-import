# Repository Guidelines

## Project Structure & Module Organization

This repository contains a Rust CLI/agent for importing files into a local vault from the terminal or Windows Explorer context menus. The original scaffold reference is `kbintake_rust_scaffold.md`; active implementation lives under `kbintake/`.

Keep the Rust crate organized by responsibility:

- `kbintake/Cargo.toml` for package metadata and dependencies.
- `kbintake/src/main.rs` as the CLI entry point.
- `kbintake/src/cli/`, `config/`, `db/`, `domain/`, `queue/`, `processor/`, `adapter/`, `agent/`, and `logging/` for focused modules.
- `kbintake/scripts/` for Windows context-menu registry files such as `register_file_context_menu.reg`.
- Put integration tests in `kbintake/tests/` and unit tests next to the module they cover.

## Build, Test, and Development Commands

Run commands from `kbintake/` after installing the Rust toolchain:

- `cargo build` compiles the CLI/agent.
- `cargo run -- import <path>` runs a local import flow.
- `cargo run -- doctor` checks configuration and local storage setup.
- `cargo test` runs unit and integration tests.
- `cargo fmt` formats Rust code.
- `cargo clippy -- -D warnings` enforces lint cleanliness before review.

## Coding Style & Naming Conventions

Use Rust 2021 style. Format all Rust code with `rustfmt`; do not hand-align large blocks. Use four-space indentation in Rust and TOML files. Prefer `snake_case` for modules, functions, variables, and filenames; use `PascalCase` for structs, enums, and traits. Keep module names aligned with responsibilities from the scaffold, for example `processor/scanner.rs` for path expansion and `queue/repository.rs` for SQLite persistence.

## Testing Guidelines

Use Rust's built-in test framework. Add focused unit tests for pure logic such as scanning, validation, state transitions, hashing, and deduplication. Add integration tests for CLI behavior, database initialization, and queue persistence. Name tests by behavior, for example `rejects_missing_input_paths` or `marks_failed_item_retryable`.

## Commit & Pull Request Guidelines

Current history only shows `Initial commit`, so no detailed convention exists yet. Use short imperative commit subjects, for example `Add queue repository tests` or `Implement import command`.

Pull requests should include a concise description, testing performed, and any Windows-specific validation steps. Link related issues when available. Include screenshots or command output when changing CLI UX, registry scripts, or setup behavior.

## V2 Planning Alignment

For KBIntake v2.0 work, do not treat local implementation order alone as the source of truth.

- Product scope source of truth: GitHub issue `#53` and `docs/PRD.md`.
- Phase tracking source of truth: GitHub issue `#54` and `docs/V2_DEVELOPMENT_PLAN.md`.
- Repo-local implementation tracking source of truth: `docs/V2_ISSUE_MAP.md`.

Before starting or continuing v2.0 development:

- check which Phase/Epic/issue the change belongs to
- prefer developing against a specific open issue or clearly documented acceptance slice
- verify that issue numbers referenced in older planning notes do not conflict with later documentation issues
- update `docs/V2_ISSUE_MAP.md` when implementation status materially changes

## Security & Configuration Tips

Do not commit local vault paths, generated SQLite databases, logs, or machine-specific registry exports. Treat registry scripts as privileged setup artifacts: review exact keys and executable paths before testing or requesting review.
