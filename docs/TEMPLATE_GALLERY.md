# KBIntake Template Gallery

Ready-to-use import templates for common knowledge management scenarios. Copy any template into your `config.toml` to get started.

## Quick Reference

### Built-in Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `{{file_name}}` | Filename without extension | `attention_is_all_you_need` |
| `{{file_ext}}` | Extension with dot | `.pdf` |
| `{{file_size_kb}}` | Size in KB (integer) | `2048` |
| `{{imported_at}}` | ISO 8601 timestamp | `2025-01-15T14:30:00+08:00` |
| `{{imported_at_date}}` | Date only | `2025-01-15` |
| `{{source_path}}` | Full source path | `C:\Users\you\Downloads\paper.pdf` |
| `{{sha256}}` | SHA-256 hash | `a1b2c3d4...` |
| `{{target_name}}` | Target vault name | `my-vault` |
| `{{batch_id}}` | Batch UUID | `550e8400-...` |

### Routing Rule Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `extension` | string or array | No | File extensions to match (e.g., `".pdf"` or `[".md", ".txt"]`) |
| `source_folder` | string | No | Match files from this directory |
| `file_name_contains` | string | No | Match files whose name contains this |
| `file_size_kb_gt` | integer | No | Min file size in KB |
| `file_size_kb_lt` | integer | No | Max file size in KB |
| `template` | string | **Yes** | Template name to apply |
| `target` | string | No | Target to route to (falls back to default) |

### Conditional Operators

`==` `!=` `>` `>=` `<` `<=` `contains` `&&` `||`

---

## 1. Research Paper

Routes PDFs from Downloads into a `references/papers/` subfolder with status tracking.

```toml
[[templates]]
name = "research-paper"
subfolder = "references/papers"
tags = ["unread", "research"]

[templates.frontmatter]
type = "paper"
status = "unread"
source = "{{source_path}}"
date = "{{imported_at_date}}"
sha256 = "{{sha256}}"

[[routing_rules]]
extension = ".pdf"
source_folder = "Downloads"
template = "research-paper"
```

Tip: The `status` field works as a workflow marker (unread -> reading -> done). Use Dataview to build a reading queue.

---

## 2. Meeting Notes

Matches files with "meeting" in the name across common text formats.

```toml
[[templates]]
name = "meeting-notes"
subfolder = "meetings"
tags = ["meeting", "unprocessed"]

[templates.frontmatter]
type = "meeting"
date = "{{imported_at_date}}"
source = "{{source_path}}"

[[routing_rules]]
extension = [".docx", ".md", ".txt"]
file_name_contains = "meeting"
template = "meeting-notes"
```

---

## 3. Quick Capture

Minimal template for any file type. Good as a default fallback.

```toml
[[templates]]
name = "quick-capture"
subfolder = "inbox"
tags = ["inbox"]

[templates.frontmatter]
type = "capture"
imported_at = "{{imported_at}}"
file_type = "{{file_ext}}"
```

Works best with Watch Mode monitoring a desktop or downloads folder.

---

## 4. Book Notes

Uses conditional logic to distinguish ebooks from plain text notes.

```toml
[[templates]]
name = "book-notes"
subfolder = "books"
tags = ["book", "reading"]

[templates.frontmatter]
type = "book"
status = "to-read"
date = "{{imported_at_date}}"
format = '''
{{#if file_ext == ".epub"}}
ebook
{{#else}}
note
{{/if}}
'''

[[routing_rules]]
extension = [".epub", ".md", ".txt"]
template = "book-notes"
```

---

## 5. Code Snippets

Tags code files with their language extension automatically.

```toml
[[templates]]
name = "code-snippet"
subfolder = "snippets"
tags = ["code"]

[templates.frontmatter]
type = "code"
language = "{{file_ext}}"
source = "{{source_path}}"

[[routing_rules]]
extension = [".py", ".rs", ".js", ".ts", ".sh", ".sql", ".go", ".java", ".c", ".cpp"]
template = "code-snippet"
```

---

## 6. Images & Screenshots

Detects large images and adds a compression reminder note.

```toml
[[templates]]
name = "image-screenshot"
subfolder = "assets/images"
tags = ["image"]

[templates.frontmatter]
type = "image"
file_name = "{{file_name}}"
size_kb = "{{file_size_kb}}"
note = '''
{{#if file_size_kb > 500}}
large-image -- consider compressing before embedding
{{/if}}
'''

[[routing_rules]]
extension = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"]
template = "image-screenshot"
```

Search for `large-image` in your vault to batch-identify files that need compression.

---

## 7. Project References (Template Inheritance)

Demonstrates single-level template inheritance. Define a base template once, then derive project-specific versions with minimal config.

```toml
# Base template
[[templates]]
name = "base-reference"
subfolder = "references/general"
tags = ["reference"]

[templates.frontmatter]
type = "reference"
status = "active"
source = "{{source_path}}"
date = "{{imported_at_date}}"
sha256 = "{{sha256}}"

# Derived template — only override what differs
[[templates]]
name = "project-alpha"
base_template = "base-reference"
subfolder = "projects/alpha"
tags = ["project-alpha"]
```

Inheritance rules:
- `frontmatter`: inherited, child fields override parent by key
- `subfolder`: child completely replaces parent value
- `tags`: merged with deduplication

Add new projects by copying the child block and changing `name`, `subfolder`, and `tags`.

---

## Share Your Templates

Share templates in [GitHub Discussions](https://github.com/GeziP/windows-rightclick-vault-import/discussions). Include:
- Template name
- Use case
- Complete TOML config
- Known limitations
