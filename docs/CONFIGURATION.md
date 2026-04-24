# KBIntake Configuration Reference

KBIntake stores configuration in:

```text
%LOCALAPPDATA%\kbintake\config.toml
```

Use this command to print the current effective config:

```powershell
kbintake config show
```

## Current Structure

```toml
app_data_dir = "C:\\Users\\<user>\\AppData\\Local\\kbintake"

[import]
max_file_size_mb = 512
inject_frontmatter = true

[agent]
poll_interval_secs = 5

[[targets]]
target_id = "default"
name = "default"
root_path = "C:\\Users\\<user>\\AppData\\Local\\kbintake\\vault"
status = "active"

[[routing]]
extensions = [".pdf", ".docx"]
target = "archive"
```

## Top-Level Fields

### `app_data_dir`

- Type: string path
- Default: `%LOCALAPPDATA%\kbintake`

This is the root directory used for:

- `config.toml`
- `data\kbintake.db`
- the default `vault\` target

For isolated runs and tests, you can override it per process:

```powershell
$env:KBINTAKE_APP_DATA_DIR = "D:\temp\kbintake"
```

## `[import]`

### `max_file_size_mb`

- Type: integer
- Default: `512`

Files larger than this limit are rejected during import or dry-run validation.

### `inject_frontmatter`

- Type: boolean
- Default: `true`

When `true`, imported Markdown files receive KBIntake frontmatter fields:

- `kbintake_source`
- `kbintake_imported_at`
- `kbintake_sha256`
- `kbintake_target`

Set to `false` if you want Markdown copied without modification.

## `[[targets]]`

Each target is a vault destination KBIntake can import into.

Fields:

- `target_id`: stable CLI identifier
- `name`: user-facing target name
- `root_path`: directory path
- `status`: `active` or `archived`

The first active target in the list is the default target.

Useful commands:

```powershell
kbintake targets list
kbintake targets add archive D:\ArchiveVault
kbintake targets set-default archive
kbintake targets rename archive references
kbintake targets remove references
```

## `[agent]`

### `poll_interval_secs`

- Type: integer
- Default: `5`

When KBIntake is running in Windows Service mode, this controls how long the background worker waits between queue polls when no work is available.

Useful commands:

```powershell
kbintake service install
kbintake service start
kbintake service status
```

Notes:

- Service management currently requires an elevated Administrator PowerShell session.
- SQLite WAL mode is enabled automatically while KBIntake is running so CLI reads can coexist with background processing.

## `[[routing]]`

Routing rules map file extensions to targets.

Example:

```toml
[[routing]]
extensions = [".pdf", ".docx", ".xlsx"]
target = "archive"

[[routing]]
extensions = [".md", ".txt"]
target = "notes"
```

Rules are evaluated in order.

Behavior:

1. If `import --target <name>` is passed, that wins.
2. Otherwise the first matching routing rule wins.
3. If no rule matches, KBIntake uses the default target.

Notes:

- Extension matching is case-insensitive.
- Extensions can be written with or without a leading dot.
- `doctor` warns if a routing rule points at a missing target.

## Common Tasks

### Change the default target

```powershell
kbintake config set-target C:\Users\<user>\Documents\KnowledgeVault
```

### Add another target and route PDFs there

```powershell
kbintake targets add archive D:\ArchiveVault
```

Then add this to `config.toml`:

```toml
[[routing]]
extensions = [".pdf"]
target = "archive"
```

### Disable Markdown frontmatter injection

```toml
[import]
max_file_size_mb = 512
inject_frontmatter = false
```

## Validation

After editing `config.toml`, run:

```powershell
kbintake doctor
```

If you changed routing rules, `doctor` will warn about rules that point to missing targets.
