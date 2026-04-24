# KBIntake

KBIntake is a Windows tool for sending files and folders into a local knowledge-base vault. You can use it from PowerShell or from the Windows Explorer right-click menu.

## Install

Preferred install path for regular Windows users:

1. Open the project's GitHub Releases page.
2. Download `KBIntake-Setup.exe`.
3. Run the installer.

After installation, KBIntake is copied into your user profile, added to your user PATH, and the Explorer right-click entries are registered for files and folders.

Planned:

- `winget install GeziP.KBIntake`

Build-from-source instructions are still available below for development and local validation.

## Quick Start

1. Install KBIntake.
2. Right-click a file in Explorer and choose the KBIntake action.
3. Open PowerShell and run:

```powershell
kbintake jobs list
```

That shows the recent import batch and its status.

If you prefer the terminal flow, this is the shortest path:

```powershell
kbintake doctor --fix
kbintake import --process C:\path\to\note.md
kbintake jobs list
```

See [docs/INSTALL.md](docs/INSTALL.md) for the fuller walkthrough.

## What KBIntake Does

- Imports one or more files or folders into a local vault target.
- Queues work in SQLite so imports can be inspected and retried.
- Deduplicates by SHA-256 hash per target.
- Supports multiple targets plus extension-based routing rules.
- Adds KBIntake frontmatter to imported Markdown files by default.
- Supports dry-run previews, undo, job inspection, vault stats, and Explorer integration.

## Command Reference

```text
kbintake doctor [--fix] [--migrate]
kbintake config show
kbintake config set-target <path> [--name <name>]
kbintake config-show
kbintake targets list [--include-archived]
kbintake targets show <target>
kbintake targets add <name> <path>
kbintake targets rename <target> <new-name>
kbintake targets remove <target> [--force]
kbintake targets set-default <target>
kbintake explorer install [--exe-path <path>] [--icon-path <path>] [--queue-only]
kbintake explorer uninstall
kbintake import [--target <target>] [--process] [--dry-run] [--json] <path...>
kbintake agent
kbintake service install
kbintake service start
kbintake service stop
kbintake service uninstall
kbintake service status
kbintake jobs list [--status <status>] [--limit <n>] [--json] [--table]
kbintake jobs show <batch-id> [--json] [--table]
kbintake jobs retry <batch-id>
kbintake jobs undo <batch-id> [--force]
kbintake vault stats [--target <target>] [--json]
```

## Configuration

KBIntake stores its runtime state in `%LOCALAPPDATA%\kbintake` by default:

- `config.toml`
- `data\kbintake.db`
- `vault\`

The main settings are:

- target list and default target
- `[import].max_file_size_mb`
- `[import].inject_frontmatter`
- `[agent].poll_interval_secs`
- `[[routing]]` extension rules

Full reference:

- [docs/CONFIGURATION.md](docs/CONFIGURATION.md)

## Vault Stats

Use `vault stats` for a per-target snapshot:

```powershell
kbintake vault stats
kbintake vault stats --target archive
kbintake vault stats --json
```

## Troubleshooting

Start with:

```powershell
kbintake doctor
```

Useful fixes:

- Missing target directory: `kbintake doctor --fix`
- Wrong vault target: `kbintake config set-target <path>`
- Missing Explorer menu: `kbintake explorer install`
- Schema mismatch after upgrades: `kbintake doctor --migrate`
- Service install/start access denied: open an elevated Administrator PowerShell before running `kbintake service install`
- PATH not updated: restart PowerShell, or add `%LOCALAPPDATA%\Programs\kbintake` to your user PATH

## Build From Source

Install Rust from <https://rustup.rs>, then build from `kbintake/`:

```powershell
cd kbintake
cargo build --release
```

For a stable per-user development install:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
Copy-Item .\assets\kbintake.ico "$env:LOCALAPPDATA\Programs\kbintake\kbintake.ico" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor --fix
```

## Windows Explorer Integration

Register context-menu entries:

```powershell
kbintake explorer install
```

Verify:

```powershell
reg query "HKCU\Software\Classes\*\shell\KBIntake\command"
reg query "HKCU\Software\Classes\Directory\shell\KBIntake\command"
```

Remove them:

```powershell
kbintake explorer uninstall
```

The reviewable `.reg` fallbacks are still in `kbintake/scripts/`.

## Project Documents

- [Install guide](docs/INSTALL.md)
- [Configuration reference](docs/CONFIGURATION.md)
- [Product Requirements Document (v2)](docs/PRD.md)
- [Contributor guide](AGENTS.md)
- [Development roadmap](docs/ROADMAP.md)
- [Release checklist](docs/RELEASE_CHECKLIST.md)
- [Rust scaffold](kbintake_rust_scaffold.md)
