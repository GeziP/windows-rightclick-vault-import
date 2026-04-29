# Contributing to KBIntake

Thanks for your interest in KBIntake! We welcome bug reports, feature requests, pull requests, template contributions, and documentation improvements. English or Chinese submissions are both welcome.

## Development Setup

### Prerequisites

| Tool | Requirement | Notes |
|------|-------------|-------|
| Rust stable toolchain | Latest stable | Install via [rustup](https://rustup.rs/) |
| Target | `x86_64-pc-windows-msvc` | MSVC toolchain required |
| OS | **Windows 10 or 11** | Required — COM, registry, and toast APIs cannot be tested cross-platform |
| Visual Studio Build Tools | 2019 or 2022 | Provides MSVC linker and Windows SDK |

### Quick Start

```bash
git clone https://github.com/GeziP/windows-rightclick-vault-import.git
cd windows-rightclick-vault-import
cargo build

# Run tests
cargo test --locked

# Pre-PR checks (all must pass)
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
```

## Project Architecture (v2.0)

```
kbintake/src/
├── main.rs           # CLI entry point, command dispatch
├── cli/mod.rs        # All CLI command handlers
├── app.rs            # Bootstrap: config load, DB init
├── config/mod.rs     # AppConfig (TOML), targets, routing, templates
├── db/               # SQLite migrations and schema
├── domain/           # Data types: BatchJob, ItemJob, ManifestRecord
├── queue/            # SQLite repository + state machine
├── processor/        # scanner, hasher, deduper, copier, validator,
│                    # frontmatter, template, dry_run, audit
├── agent/            # worker (import pipeline), scheduler (queue drain),
│                    # watcher (file system watcher)
├── adapter/          # local_folder storage adapter
├── explorer/         # Windows registry context menu + COM probe
├── tray/             # System tray icon + auto-start management
├── service.rs        # Windows Service lifecycle
├── notify.rs         # Windows toast notifications
├── i18n.rs           # en / zh-CN localization
└── exit_codes.rs     # Structured exit codes
```

All import paths (CLI, Explorer right-click, Watch Mode) converge on the same processing pipeline in `agent/worker.rs`, ensuring consistent behavior.

## Development Workflow

### Branch Naming

| Type | Format | Example |
|------|--------|---------|
| Feature | `feat/short-description` | `feat/template-engine` |
| Bug fix | `fix/short-description` | `fix/frontmatter-encoding` |
| Docs | `docs/short-description` | `docs/config-reference` |
| i18n | `i18n/short-description` | `i18n/zh-cn-tui` |

### Process

1. Pick an issue and comment to claim it
2. Fork and create a branch from the latest `main` or `v2.0`
3. Develop with tests — `cargo test --locked` must pass
4. Run quality checks: `cargo fmt` + `cargo clippy -- -D warnings`
5. Open a PR with the issue reference, change description, and test plan
6. Address review feedback, then squash merge

## Code Conventions

### Rust Style

- `cargo fmt` formatted, `cargo clippy` zero warnings
- Avoid `.unwrap()` and `.expect()` in production code paths — use `anyhow::Result`
- Exit codes defined in `src/exit_codes.rs`

### Localization

All user-visible strings must use the `tr()` function from `src/i18n.rs`:

```rust
// Correct
println!("{}", tr("import.success", lang));

// Wrong — no hardcoded strings
println!("Successfully imported");
```

When adding new strings, update both the English and Chinese dictionaries in `src/i18n.rs`.

### Database Changes

- Schema migrations are append-only in `src/db/schema.rs`
- Bump `LATEST_SCHEMA_VERSION` in `src/db/mod.rs`
- Update the migration test assertion in `tests/mvp_flow.rs`
- Never modify existing migration SQL

## Testing

### Unit Tests

Live alongside modules as `#[cfg(test)] mod tests`. New features should cover:
- Happy path
- Failure path (invalid input, missing file)
- Edge cases (empty strings, Unicode filenames, boundary values)

### Integration Tests

Located in `kbintake/tests/`. Use `tempfile` for isolated test directories.

Windows-specific tests should be gated:

```rust
#[test]
#[cfg(target_os = "windows")]
fn test_context_menu_registration() { ... }
```

## Reporting Issues

### Bug Reports

Include:
- Windows version (e.g., Windows 11 23H2 Build 22631)
- KBIntake version (`kbintake --version`)
- Exact reproduction steps
- Expected vs actual behavior with full error messages
- `kbintake doctor` output

### Feature Requests

Include a user story ("As a X, I want Y, so that Z") and a concrete usage scenario.

## Community Templates

Share your import templates in [GitHub Discussions](https://github.com/GeziP/windows-rightclick-vault-import/discussions). Format:

1. Template name
2. Use case description
3. Complete TOML config block
4. Known limitations
