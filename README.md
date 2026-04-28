# KBIntake

KBIntake is a Windows-first local vault importer. It lets you send files and folders into a knowledge-base vault from PowerShell or from the Windows Explorer right-click menu, while keeping an auditable SQLite job history.

Current release: `v2.0.0` (in development on branch `v2.0`)

- Download: <https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0>
- Chinese README: [README.zh-CN.md](README.zh-CN.md)

## What It Is For

KBIntake is built for people who collect notes, PDFs, screenshots, exports, and reference files throughout the day and want a repeatable way to move them into a local vault. Instead of manually copying files, checking for duplicates, renaming conflicts, and remembering where each file went, KBIntake records every import as a job and stores the resulting file manifest.

The default target is a local folder vault, such as:

```text
C:\Users\<you>\Documents\KBIntakeVault
```

KBIntake does not require a cloud service. Configuration, queue state, manifests, logs, and the default vault are local to your Windows profile.

## Install

### Recommended

1. Open the [v1.0.0 release page](https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0).
2. Download `KBIntake-Setup.exe`.
3. Run the installer.
4. Open a new PowerShell window and run:

```powershell
kbintake doctor
```

The installer:

- installs `kbintake.exe`, `kbintakew.exe`, and `kbintake.ico` under `%LOCALAPPDATA%\Programs\kbintake`
- adds that directory to your user `PATH`
- registers Explorer right-click entries for files and folders
- creates a Windows Settings uninstall entry

### Winget Status

The winget manifest is prepared and validated in `installer/winget/1.0.0`, and the community package PR has been submitted: <https://github.com/microsoft/winget-pkgs/pull/364698>.

Until that PR is merged into the public winget source, use the GitHub Release installer.

Expected command after publication:

```powershell
winget install GeziP.KBIntake
```

## Quick Start

Explorer flow:

1. Right-click a file or folder.
2. Choose the KBIntake action to expand the submenu.
3. Select Import (process immediately), Queue (queue for later), or Settings (open TUI).
4. KBIntake imports it silently and shows a Windows toast notification.
5. Inspect the result:

```powershell
kbintake jobs list
```

Terminal flow:

```powershell
kbintake doctor --fix
kbintake import --process C:\path\to\note.md
kbintake jobs list
```

## Features

- Explorer cascading right-click menu with Import / Queue / Settings sub-items
- Windows 11 native top-level context menu via COM DLL
- no-console Explorer flow through `kbintakew.exe`
- Windows toast notifications for success, duplicate, and failure cases
- terminal import flow with optional immediate processing
- SQLite-backed batches, items, manifests, and audit events
- SHA-256 hashing and per-target duplicate detection
- deterministic filename conflict handling without overwriting existing files
- multiple vault targets with add/list/show/rename/remove/set-default commands
- import template system with variable interpolation and conditional rendering
- v2 multi-condition routing rules with template binding
- per-target default subfolder configuration
- Markdown frontmatter injection with template-defined fields
- `--tags` quick tag injection merged with template tags
- `--clipboard` import file paths from Windows clipboard
- dry-run preview with table or JSON output
- job list/show with table or JSON output
- retry failed jobs
- hash-safe undo for imported batches
- per-target vault statistics
- `vault audit` command for orphan, missing, duplicate, and malformed frontmatter detection
- Watch Mode: monitor directories for new files with debounce and extension filters
- TUI interactive settings (`kbintake tui`)
- Obsidian URI integration with auto-open after import
- zh-CN (Chinese) localization (CLI, toast, TUI, Explorer context menu)
- Windows Service mode for background queue processing
- release workflow that publishes installer and binary assets
- winget manifest copy stored under `installer/winget/`

## Common Commands

```text
kbintake --version
kbintake version
kbintake doctor [--fix] [--migrate]
kbintake config show
kbintake config validate
kbintake config set-target <path> [--name <name>]
kbintake config-show
kbintake targets list [--include-archived]
kbintake targets show <target>
kbintake targets add <name> <path>
kbintake targets rename <target> <new-name>
kbintake targets remove <target> [--force]
kbintake targets set-default <target>
kbintake import [--target <target>] [--template <name>] [--tags "a,b"] [--clipboard] [--process] [--dry-run] [--json] [--open] <path...>
kbintake jobs list [--status <status>] [--limit <n>] [--json] [--table]
kbintake jobs show <batch-id> [--json] [--table]
kbintake jobs retry <batch-id>
kbintake jobs undo <batch-id> [--force]
kbintake vault stats [--target <target>] [--json]
kbintake vault audit [--target <target>] [--fix] [--json]
kbintake watch [--path <dir>]
kbintake tui
kbintake obsidian open --vault <name> <note-path>
kbintake explorer install [--exe-path <path>] [--icon-path <path>] [--queue-only]
kbintake explorer uninstall
kbintake agent
kbintake service install
kbintake service start
kbintake service stop
kbintake service uninstall
kbintake service status
```

## Configuration

Runtime state lives in `%LOCALAPPDATA%\kbintake` by default:

- `config.toml`
- `data\kbintake.db`
- `logs\`
- `vault\`

Important config sections:

- `[[targets]]`: vault destinations (with optional `default_subfolder`, `obsidian_vault`)
- `[[templates]]`: import templates with subfolder, tags, and frontmatter
- `[[routing_rules]]`: v2 multi-condition routing with template binding
- `[[routing]]`: v1 extension-based routing (still supported)
- `[[watch]]`: directories to monitor for automatic import
- `[import].max_file_size_mb`: file size guardrail
- `[import].inject_frontmatter`: Markdown metadata injection
- `[import].language`: output language (`"en"` or `"zh-CN"`), also controls Explorer context menu text
- `[import].auto_open_obsidian`: auto-open imported notes in Obsidian
- `[agent].poll_interval_secs`: background worker polling interval

Full reference: [docs/CONFIGURATION.md](docs/CONFIGURATION.md)

## Background Processing

For terminal use, `kbintake import --process <path>` queues and processes immediately.

For passive background processing, install the Windows Service from an elevated Administrator PowerShell:

```powershell
kbintake service install
kbintake service start
kbintake service status
```

To remove it:

```powershell
kbintake service stop
kbintake service uninstall
```

Service mode is implemented and validated for install/start/queue processing/logging/stop/uninstall. Reboot-resume validation remains a release-checklist manual item.

## Troubleshooting

Start with:

```powershell
kbintake doctor
```

Common fixes:

- Missing target directory: `kbintake doctor --fix`
- Wrong vault target: `kbintake config set-target <path>`
- Explorer menu missing: `kbintake explorer install`
- Schema needs migration: `kbintake doctor --migrate`
- `kbintake` not found after install: open a new PowerShell window
- Service install/start access denied: use Administrator PowerShell

## Build From Source

Install Rust from <https://rustup.rs>, then:

```powershell
cd kbintake
cargo build --release --locked --bins
```

To build the installer locally, install NSIS and run from the repository root:

```powershell
New-Item -ItemType Directory -Force .\dist | Out-Null
Copy-Item .\kbintake\target\release\kbintake.exe .\dist\kbintake.exe -Force
Copy-Item .\kbintake\target\release\kbintakew.exe .\dist\kbintakew.exe -Force
Copy-Item .\kbintake\assets\kbintake.ico .\dist\kbintake.ico -Force
& "C:\Program Files (x86)\NSIS\makensis.exe" .\installer\kbintake.nsi
```

## Validation

Automated checks:

```powershell
cd kbintake
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked
cargo build --release --locked --bins
```

Manual Windows smoke checks:

```powershell
.\scripts\validate-explorer-toast.ps1
.\scripts\validate-service-mode.ps1
```

`validate-service-mode.ps1` requires an elevated Administrator PowerShell session.

## Planned Work

- v2.0 release: installer update, version bump, CHANGELOG, release notes
- Windows 11 COM DLL physical machine validation
- monitor the `microsoft/winget-pkgs` PR for winget publication
- code-sign release binaries to reduce SmartScreen friction
- documentation pass for v2.0 (template gallery, config reference update)
- perform reboot-resume validation for service mode

See [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

## Project Documents

- [Chinese README](README.zh-CN.md)
- [Install guide](docs/INSTALL.md)
- [Configuration reference](docs/CONFIGURATION.md)
- [Project status](docs/PROJECT_STATUS.md)
- [Release checklist](docs/RELEASE_CHECKLIST.md)
- [Development roadmap](docs/ROADMAP.md)
- [Product requirements](docs/PRD.md)
- [Contributor guide](AGENTS.md)
