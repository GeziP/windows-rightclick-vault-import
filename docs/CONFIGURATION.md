# KBIntake Configuration Reference

KBIntake stores configuration in:

```text
%LOCALAPPDATA%\kbintake\config.toml
```

Use this command to print the current effective config:

```powershell
kbintake config show
```

## Full Example

```toml
app_data_dir = "C:\\Users\\<user>\\AppData\\Local\\kbintake"

[import]
max_file_size_mb = 512
inject_frontmatter = true
language = "en"
auto_open_obsidian = false

[agent]
poll_interval_secs = 5
watch_in_service = false

[[targets]]
target_id = "default"
name = "default"
root_path = "C:\\Users\\<user>\\AppData\\Local\\kbintake\\vault"
status = "active"
default_subfolder = "inbox"
obsidian_vault = "MyVault"

[[routing]]
extensions = [".pdf", ".docx"]
target = "archive"

[[templates]]
name = "notes"
subfolder = "notes/{{imported_at_date}}"
tags = ["imported"]
frontmatter = """
title: "{{file_name}}"
tags:
  - imported
"""

[[templates]]
name = "pdf-archive"
base_template = "notes"
subfolder = "archive/pdf"

[[routing_rules]]
extensions = [".md", ".txt"]
target = "default"
template = "notes"

[[routing_rules]]
extensions = [".pdf"]
file_size_range = [0, 104857600]
target = "default"
template = "pdf-archive"

[[watch]]
path = "C:\\Users\\<user>\\Downloads"
target = "default"
template = "notes"
extensions = [".md", ".txt"]
debounce_secs = 3
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

### `language`

- Type: string
- Default: `"en"`
- Options: `"en"`, `"zh-CN"`

Sets the language for CLI output, toast notifications, TUI labels, and Explorer right-click menu text.

When you change this value, run `kbintake explorer install` to update the registry menu text to match the new language.

### `auto_open_obsidian`

- Type: boolean
- Default: `false`

When `true`, imported Markdown files are automatically opened in Obsidian after a successful import. Requires `obsidian_vault` to be set on the target.

## `[agent]`

### `poll_interval_secs`

- Type: integer
- Default: `5`

When KBIntake is running in Windows Service mode, this controls how long the background worker waits between queue polls when no work is available.

### `watch_in_service`

- Type: boolean
- Default: `false`

When `true`, the Windows Service starts a watcher thread alongside the queue processor. Watch paths are taken from `[[watch]]` config entries.

## `[[targets]]`

Each target is a vault destination KBIntake can import into.

Fields:

- `target_id`: stable CLI identifier (auto-generated from name)
- `name`: user-facing target name
- `root_path`: directory path
- `status`: `active` or `archived`
- `default_subfolder`: optional subfolder for imports without a template subfolder
- `obsidian_vault`: optional Obsidian vault name for auto-open

The first active target in the list is the default target.

Useful commands:

```powershell
kbintake targets list
kbintake targets add archive D:\ArchiveVault
kbintake targets set-default archive
kbintake targets rename archive references
kbintake targets remove references
```

## `[[routing]]` (v1)

Routing rules map file extensions to targets.

Example:

```toml
[[routing]]
extensions = [".pdf", ".docx", ".xlsx"]
target = "archive"
```

Rules are evaluated in order. v1 routing is still supported but v2 `routing_rules` take priority when both exist.

## `[[templates]]`

Templates define how imports are processed: destination subfolder, tags, and frontmatter fields.

Fields:

- `name`: unique template name (required)
- `base_template`: inherit settings from another template (optional, single-level only)
- `subfolder`: destination subfolder with variable interpolation
- `tags`: list of tags to inject into frontmatter
- `frontmatter`: multi-line TOML string for custom frontmatter fields

### Variable Interpolation

Available in `subfolder` and `frontmatter`:

| Variable | Description |
|----------|-------------|
| `{{file_name}}` | Source filename without extension |
| `{{file_ext}}` | File extension (e.g., `.md`) |
| `{{file_size_kb}}` | File size in kilobytes |
| `{{imported_at}}` | ISO 8601 timestamp |
| `{{imported_at_date}}` | Date only (YYYY-MM-DD) |
| `{{source_path}}` | Full source file path |
| `{{sha256}}` | SHA-256 hash of the file |
| `{{target_name}}` | Target name |
| `{{batch_id}}` | Batch identifier |

### Conditional Rendering

Use `{{#if}}` blocks in frontmatter:

```toml
frontmatter = """
{{#if file_ext == ".md"}}
type: markdown
{{#else}}
type: document
{{/if}}
"""
```

Supported operators: `==`, `!=`, `>`, `>=`, `<`, `<=`, `contains`, `&&`, `||`.

### Inheritance

```toml
[[templates]]
name = "base"
subfolder = "inbox"
tags = ["imported"]

[[templates]]
name = "urgent"
base_template = "base"
tags = ["imported", "urgent"]
```

Child templates override parent fields. Omitted fields are inherited.

## `[[routing_rules]]` (v2)

Multi-condition routing rules with template binding.

Fields:

- `extensions`: list of file extensions to match (case-insensitive)
- `source_folder`: match files from a specific directory
- `file_name_contains`: match files whose name contains this string
- `file_size_range`: `[min_bytes, max_bytes]` range
- `target`: target ID or name to route to
- `template`: template name to apply

Example:

```toml
[[routing_rules]]
extensions = [".md"]
target = "notes"
template = "notes"
```

Rules are evaluated in order. First match wins. If no `target` is specified, falls back to v1 `routing` or the default target.

## `[[watch]]`

Watch Mode monitors directories for new files and imports them automatically.

Fields:

- `path`: directory to watch (required)
- `target`: target ID or name
- `template`: template name to apply
- `extensions`: list of extensions to import (omit for all)
- `debounce_secs`: seconds to wait after last write before importing (default: 3)

Example:

```toml
[[watch]]
path = "C:\\Users\\<user>\\Downloads"
target = "default"
extensions = [".md", ".txt"]
debounce_secs = 5
```

Start watching:

```powershell
kbintake watch
```

## CLI Import Flags

### `--tags`

Comma-separated tags injected into frontmatter, merged with template tags (case-insensitive dedup):

```powershell
kbintake import note.md --tags "urgent,project-alpha" --process
```

### `--clipboard`

Read file paths from Windows clipboard (one per line) and import them:

```powershell
kbintake import --clipboard --process
```

Can be combined with other flags:

```powershell
kbintake import --clipboard --tags "from-clipboard" --dry-run --json
```

### `--template` / `-t`

Override the resolved template for this import:

```powershell
kbintake import report.pdf --template pdf-archive --process
```

### `--open`

Open imported Markdown notes in Obsidian after import:

```powershell
kbintake import note.md --process --open
```

## Common Tasks

### Change the default target

```powershell
kbintake config set-target C:\Users\<user>\Documents\KnowledgeVault
```

### Add another target and route PDFs there

```powershell
kbintake targets add archive D:\ArchiveVault
```

Then add to `config.toml`:

```toml
[[routing]]
extensions = [".pdf"]
target = "archive"
```

### Disable Markdown frontmatter injection

```toml
[import]
inject_frontmatter = false
```

### Set Chinese language output

```toml
[import]
language = "zh-CN"
```

Then update the Explorer menu text:

```powershell
kbintake explorer install
```

### Auto-open notes in Obsidian

```toml
[import]
auto_open_obsidian = true
```

And set `obsidian_vault` on your target:

```toml
[[targets]]
name = "default"
root_path = "C:\\Users\\<user>\\ObsidianVault"
obsidian_vault = "MyVault"
```

## Validation

After editing `config.toml`, run:

```powershell
kbintake doctor
kbintake config validate
```

`doctor` warns about rules that point to missing targets. `config validate` checks template references and config semantics.
