# KBIntake

KBIntake is a Windows-friendly Rust CLI for importing files and folders into a local knowledge-base vault. It is designed to work from both PowerShell and Windows Explorer right-click menus.

The current release is a local MVP: it scans files, queues import jobs in SQLite, processes them into configured vault folders, records manifests/events, and exposes enough job inspection and retry commands for day-to-day testing.

## Features

- Import one or more files or folders from the terminal.
- Recursively scan directories without following symlinks.
- Queue imports in a local SQLite database.
- Process queued work with `agent` or immediately with `import --process`.
- Validate source files, enforce max size, compute SHA-256 hashes, and deduplicate by target/hash.
- Copy files into local vault targets without overwriting existing files.
- Manage multiple vault targets with `targets add/list/show/rename/remove/set-default`.
- Import into a specific target with `import --target <target>`.
- Inspect batches and item details with `jobs list` and `jobs show`.
- Retry failed items with `jobs retry <batch-id>`.
- Record audit events for queued, success, duplicate, failed, and retry transitions.
- Register Windows Explorer file and directory context-menu entries with reviewable `.reg` scripts.
- Run CI on Windows for format, clippy, build, and tests.

## Status

`v0.1.0` is suitable for local MVP validation. It is not yet packaged as an installer or Windows service.

Implemented:

- CLI import, processing, jobs, retry, config, targets, and doctor commands.
- SQLite schema, manifest records, event audit trail, and schema validation.
- Windows Explorer registration and rollback scripts.
- Unit and integration tests using temporary directories.
- Release validation checklist in [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md).

In development / planned:

- Manual v0.1 Windows Explorer validation using the release checklist.
- Registry script generation or installer flow so users do not hand-edit executable paths.
- Long-running background agent or Windows service mode.
- Schema migration versioning.
- More polished job output formats such as JSON or table views.

## Install From Source

Install the Rust toolchain from <https://rustup.rs>, then build from the crate directory:

```powershell
cd kbintake
cargo build --release
```

For a stable per-user install path:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
Copy-Item .\assets\kbintake.ico "$env:LOCALAPPDATA\Programs\kbintake\kbintake.ico" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor
```

## Quick Start

Run these commands from `kbintake/` during development:

```powershell
cargo run -- doctor
cargo run -- config show
cargo run -- config set-target C:\Users\<user>\Documents\KnowledgeVault
cargo run -- import C:\path\to\note.md
cargo run -- agent
cargo run -- jobs list
```

Import and process in one command:

```powershell
cargo run -- import --process C:\path\to\note.md
```

Add and use another target:

```powershell
cargo run -- targets add archive D:\ArchiveVault
cargo run -- targets list
cargo run -- import --target archive --process C:\path\to\folder
```

Rename or remove a target:

```powershell
cargo run -- targets rename archive notes
cargo run -- targets remove notes
```

Retry failed work:

```powershell
cargo run -- jobs show <batch-id>
cargo run -- jobs retry <batch-id>
cargo run -- agent
```

## Command Reference

```text
kbintake doctor
kbintake config show
kbintake config set-target <path> [--name <name>]
kbintake config-show
kbintake targets list
kbintake targets show <target>
kbintake targets add <name> <path>
kbintake targets rename <target> <new-name>
kbintake targets remove <target>
kbintake targets set-default <target>
kbintake explorer install [--exe-path <path>] [--icon-path <path>] [--queue-only]
kbintake explorer uninstall
kbintake import [--target <target>] [--process] <path...>
kbintake agent
kbintake jobs list
kbintake jobs show <batch-id>
kbintake jobs retry <batch-id>
```

## Runtime State

Local runtime state is created under `%LOCALAPPDATA%\kbintake` by default:

- `config.toml` stores targets and import settings.
- `data\kbintake.db` stores batches, items, manifests, and events.
- `vault\` is the default target until changed with `kbintake config set-target`.

Do not delete this directory unless you intentionally want to remove local config, queue state, manifests, and the default vault.

## Windows Explorer Integration

Build a release executable and copy it to a stable per-user path before registering context-menu entries:

```powershell
cd kbintake
cargo build --release
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
Copy-Item .\assets\kbintake.ico "$env:LOCALAPPDATA\Programs\kbintake\kbintake.ico" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor
```

Register the file and directory context menus:

```powershell
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" explorer install
```

By default, Explorer imports are processed immediately with `import --process "%1"`. Use `--queue-only` if right-click imports should only enqueue work for a later `kbintake agent` run.

Verify the registered commands:

```powershell
reg query "HKCU\Software\Classes\*\shell\KBIntake\command"
reg query "HKCU\Software\Classes\Directory\shell\KBIntake\command"
reg query "HKCU\Software\Classes\*\shell\KBIntake" /v Icon
reg query "HKCU\Software\Classes\Directory\shell\KBIntake" /v Icon
```

Roll back the context-menu entries:

```powershell
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" explorer uninstall
```

The `.reg` files remain available as reviewable fallback artifacts. Before applying them manually, review them and replace the placeholder executable and icon paths with the expanded absolute paths for your account, for example:

```text
C:\Users\<user>\AppData\Local\Programs\kbintake\kbintake.exe
C:\Users\<user>\AppData\Local\Programs\kbintake\kbintake.ico
```

The registration scripts should only modify these per-user keys:

```text
HKEY_CURRENT_USER\Software\Classes\*\shell\KBIntake
HKEY_CURRENT_USER\Software\Classes\Directory\shell\KBIntake
```

Manual registration fallback:

```powershell
reg import .\kbintake\scripts\register_file_context_menu.reg
reg import .\kbintake\scripts\register_dir_context_menu.reg
```

Manual rollback fallback:

```powershell
reg import .\kbintake\scripts\unregister_file_context_menu.reg
reg import .\kbintake\scripts\unregister_dir_context_menu.reg
```

After unregistering, the `reg query` commands above should report that the keys do not exist.

## Development

Run from `kbintake/`:

```powershell
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked
```

GitHub Actions runs the same quality gates on `windows-latest`.

## Troubleshooting

- If `cargo` is not recognized, install Rust with `rustup` and open a new PowerShell session so `PATH` is refreshed.
- If `doctor` reports a schema or database error, check that `%LOCALAPPDATA%\kbintake\data` is writable and that no other process has locked `kbintake.db`.
- If `doctor` reports a target error, run `kbintake config set-target <vault-path>` with a directory your user account can create and write to.
- If Explorer menu commands do nothing, verify the registered command path points to the installed `kbintake.exe`, not the placeholder in the `.reg` files.

## Project Documents

- [Product Requirements Document (v2)](docs/PRD.md) — **Start here before writing any code.**
- [Contributor guide](AGENTS.md)
- [Development roadmap](docs/ROADMAP.md)
- [v0.1 release checklist](docs/RELEASE_CHECKLIST.md)
- [Rust scaffold](kbintake_rust_scaffold.md)
