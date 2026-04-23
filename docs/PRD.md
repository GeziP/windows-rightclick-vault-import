# KBIntake — Product Requirements Document

### TL;DR
KBIntake is a Windows-native Rust command-line utility (kbintake.exe) that allows users to import any file or folder directly into a local knowledge-base vault (like Obsidian) using Windows Explorer right-click menus or PowerShell. It features SHA-256 deduplication, a SQLite-backed job queue, management of multiple vault targets, and detailed audit event tracking. KBIntake targets knowledge workers, researchers, and developers on Windows who value privacy and seamless, zero-cloud, vault management.

---

## Goals

### Business Goals

- Establish KBIntake as the frictionless ingestion layer of choice for local Windows knowledge-base vaults.
- Achieve v1.0 with a one-click installer, ensuring accessibility to non-developer users.
- Increase GitHub stars and foster community adoption by delivering a reliable, privacy-respecting, zero-cloud tool.
- Develop the foundation for a Windows Service background agent enabling fully passive file capture workflows.
- Position KBIntake as the leading Windows-native complement to Obsidian and similar personal knowledge management (PKM) tools.

### User Goals

- Enable file import into a vault with a single right-click, eliminating the need for terminal use.
- Ensure duplicates are transparently managed via SHA-256 deduplication.
- Allow intuitive management of multiple vault targets without manual config edits.
- Provide clear inspection, retry, and auditing of every import job from the CLI.
- Foster complete trust in privacy—guarantee all operations are local with no telemetry or cloud access.

### Non-Goals

- No support for cloud synchronization or remote vaults.
- No plans for a GUI application or system tray interface in v1.x.
- No cross-platform support for macOS or Linux in the current major version.

---

## User Stories

**Persona 1 — Knowledge Worker (Obsidian User):**
- As a knowledge worker, I want to right-click any file in Explorer and import it to my vault, so that I can capture resources without interrupting my workflow.
- As a knowledge worker, I want imported markdown files to have YAML frontmatter injected automatically, so that they are immediately queryable inside Obsidian.
- As a knowledge worker, I want duplicate files silently skipped, so that I never end up with redundant copies in my vault.

**Persona 2 — Power User / Developer:**
- As a power user, I want to define per-file-type routing rules in config.toml (e.g., .pdf → archive, .md → notes), so that files land in the right target automatically.
- As a power user, I want to run `kbintake jobs list --json` and pipe output to other tools, so that I can build automation on top of KBIntake.
- As a power user, I want to undo a batch import with `kbintake jobs undo <batch-id>`, so that I can recover from accidental bulk imports.

**Persona 3 — Non-Developer User:**
- As a non-developer, I want to install KBIntake via a single .exe installer or winget, so that I never have to touch Rust or PowerShell to get started.
- As a non-developer, I want the right-click menu to appear immediately after install with no manual registry editing, so that setup is foolproof.

---

## Functional Requirements

#### ✅ v0.1.0 — Shipped

- **Group 1 — Core Import Pipeline (Priority: P0)**
  - Import command accepts one or more file/folder paths.
  - Recursive directory scanning without following symlinks.
  - SHA-256 hash computation and deduplication per target.
  - Max file size enforcement.
  - Copy-without-overwrite into vault target.
  - `--process` flag for immediate processing.
  - Audit event recording for each job status (queued, success, duplicate, failed, retry).

- **Group 2 — Job Management Basics (Priority: P0)**
  - SQLite-backed job queue with batch/item granularity.
  - `jobs list` for batch overview.
  - `jobs show <batch-id>` for detailed, item-level status.
  - `jobs retry <batch-id>` for failed items.

- **Group 3 — Target Management (Priority: P0)**
  - `targets add/list/show/rename/remove/set-default` commands.
  - Per-import `--target` flag, with default target fallback.

- **Group 4 — Windows Explorer Integration (Priority: P0)**
  - `explorer install` registers HKCU (per-user) context-menu entries for files and folders.
  - `explorer uninstall` removes context menus.
  - `--queue-only` flag for deferred processing.
  - Installer auto-injects the exe path—no manual registry editing.

- **Group 5a — Configuration & Doctor Basics (Priority: P1)**
  - `config show` and `config set-target` commands.
  - TOML-based config at `%LOCALAPPDATA%\kbintake\config.toml`.
  - `doctor` command validates config, DB schema, and target paths.

#### 🔧 v1.0 — In Development

- **Group 2b — Job Management Enhancements (Priority: P0)**
  - `jobs list` with `--json` and `--table` output formats. (issue [#35](https://github.com/GeziP/windows-rightclick-vault-import/issues/35))
  - `jobs undo <batch-id>` to delete copied files and roll back the batch. (issue [#34](https://github.com/GeziP/windows-rightclick-vault-import/issues/34))

- **Group 5b — Configuration Enhancements (Priority: P1)**
  - File-type routing rules in config (e.g., `.pdf` to `archive`, `.md` to `notes`). (issue [#47](https://github.com/GeziP/windows-rightclick-vault-import/issues/47))
  - `doctor` command provides explicit remediation hints for detected issues. (issue [#39](https://github.com/GeziP/windows-rightclick-vault-import/issues/39))

- **Group 6 — Frontmatter Injection (Priority: P1)**
  - On import of `.md` files, prepend YAML frontmatter with: source_path, imported_at, sha256, target, file_type. (issue [#36](https://github.com/GeziP/windows-rightclick-vault-import/issues/36))

- **Group 7 — Installer & Distribution (Priority: P1)**
  - Pre-built `.exe` release artifact via GitHub Actions. (issue [#37](https://github.com/GeziP/windows-rightclick-vault-import/issues/37))
  - NSIS installer (.exe) handles exe copy, icon copy, context-menu registration, and PATH update in one step — no admin privileges required (per-user install to `%LOCALAPPDATA%`). (issue [#38](https://github.com/GeziP/windows-rightclick-vault-import/issues/38))

#### 📋 v1.x — Planned

- **Group 8 — Background Agent / Windows Service (Priority: P2)**
  - Long-running Windows Service (`kbintake service install/start/stop/uninstall`). (issue [#46](https://github.com/GeziP/windows-rightclick-vault-import/issues/46))
  - Polls queue at a configurable interval.
  - Replaces manual agent invocation.

- **Group 9 — Vault Stats (Priority: P2)**
  - `kbintake vault stats` shows file counts, storage used, duplicate rate, and recent activity per target. (issue [#41](https://github.com/GeziP/windows-rightclick-vault-import/issues/41))

---

## User Experience

**Entry Point & First-Time User Experience**
   - User downloads the installer from GitHub Releases.
   - Runs the installer: copies `kbintake.exe` to `%LOCALAPPDATA%\Programs\kbintake`.
   - Installer executes `kbintake doctor` to validate setup and registry.
   - Installer runs `kbintake explorer install` to register right-click menus.
   - No manual setup—usable immediately post-install.

**Core Experience — Right-Click Import**
   - **Step 1:** User right-clicks a file or folder in Windows Explorer.
     - Frictionless access; menu appears contextually for files and folders.
   - **Step 2:** User selects 'Import to KBIntake' from context menu.
   - **Step 3:** `kbintake.exe` runs `import --process` silently in the background.
     - No command window pops up; runs with `CREATE_NO_WINDOW`.
   - **Step 4:** On completion, Windows toast notification confirms success and shows import summary (file count, duplicates skipped).
   - **Step 5:** If any items fail, notification provides retry instructions (`kbintake jobs retry <batch-id>`).

**Core Experience — CLI Power Use**
   - **Step 1:** User runs `kbintake import --target archive --process C:\path\to\folder`.
   - **Step 2:** CLI prints detailed progress: files scanned, queued, processed, duplicates, failures.
   - **Step 3:** User reviews batches via `kbintake jobs list --table` or `--json`.
   - **Step 4:** User inspects job details with `kbintake jobs show <batch-id>`.
   - **Step 5:** User can roll back imports using `kbintake jobs undo <batch-id>` as needed.

**Advanced Features & Edge Cases**
   - Symlinks skipped silently during folder scan.
   - Files over the max size are logged as failed, with clear messaging.
   - Duplicate detection is per-target (same file can exist in multiple vaults).
   - Targets are auto-created if not present during first import.
   - `doctor` recommends explicit remediation steps for config or DB issues.

**UI/UX Highlights**
   - No visible console when running from Explorer.
   - Windows Action Center provides toast notifications for import completion and errors.
   - CLI output uses color-coding for status (success: green, duplicate: yellow, fail: red).
   - `--json` output everywhere for easy scripting.
   - All destructive actions prompt for confirmation (or require `--force`).

---

## Narrative

Maya, a research analyst, relies on Obsidian as her personal knowledge base. Frustrated by the repetitive chore of manually copying PDFs, screenshots, and notes from her Downloads folder into her vault, she finds her workflow constantly interrupted. That changes when a colleague recommends KBIntake. One download and a simple installer later, Maya's right-click menu in Explorer now features 'Import to KBIntake'—no terminal or registry tweaks required.

The next time Maya saves a research PDF, she simply right-clicks and selects 'Import to KBIntake.' Instantly, the file lands in her dedicated archive vault. When she imports markdown notes, KBIntake automatically injects YAML frontmatter, so they're instantly searchable within Obsidian's graph view. Over the following week, Maya imports hundreds of documents, never worrying about overwriting or duplicating files—KBIntake's silent SHA-256 deduplication handles everything in the background.

At the end of the week, Maya checks her stats with `kbintake vault stats`: 340 files imported, 12% duplicates skipped, zero failures. Her workflow is noticeably smoother. She's free to capture and organize knowledge without friction, and KBIntake blends quietly into her digital environment—always there, never intrusive. For Maya and users like her, KBIntake turns importing into a non-event—a seamless, invisible extension of her thinking process.

---

## Success Metrics

### User-Centric Metrics
- **Time-to-first-import** for new users under 2 minutes post-install.
- **Zero failed imports** due to registry misconfiguration after installation.
- **Duplicate skip rate** tracked per vault — with healthy usage at less than 15%.

### Business Metrics
- **100 GitHub stars** within 3 months of the v1.0 launch.
- **50+ unique installer downloads** in the first month.
- **At least 1 community contribution** (PR or issue) within 60 days of launch.

### Technical Metrics
- **Processing latency** under 500ms per file (up to 10MB) on mid-range Windows hardware.
- **Zero data loss incidents** (no unwanted file overwrites, deduplication always enforced).
- **100% CI pass rate** (formatting, linting, build, and tests) on all Windows commits.

### Tracking Plan
- `import_queued` (batch_id, file_count, target)
- `import_processed` (batch_id, success_count, duplicate_count, failed_count)
- `job_retry` (batch_id)
- `job_undo` (batch_id)
- `explorer_install` / `explorer_uninstall`
- `doctor_run` (pass/fail)

*All metrics are stored in the local SQLite audit log—no external telemetry.*

---

## Technical Considerations

### Technical Needs
- Rust binary (`kbintake.exe`) compiled for x86_64 Windows.
- SQLite for job queue, manifest, and event/audit logs.
- SHA-256 hashing for deduplication.
- TOML-based config files.
- Windows registry integration for Explorer context menu.
- YAML frontmatter injection for markdown imports.
- CLI argument parsing via Clap.

### Integration Points
- Windows Explorer context menu (HKCU registry keys only; no COM/DLL).
- Windows Action Center for notifications.
- GitHub Actions for CI/CD (artifact builds).
- winget community repository for distribution (v1.x milestone).

### Data Storage & Privacy
- All state is stored locally in `%LOCALAPPDATA%\kbintake`:
  - Config in `config.toml`
  - Database (`kbintake.db`) for batches, items, manifests, and audit events
- No telemetry, no network access, and no cloud sync
- Schema migration/versioning included for safe upgrades

### Scalability & Performance
- Designed for single-user, local use.
- Target: < 500ms per file for files up to 10MB.
- Recursive import supports thousands of files (streaming scan).
- SQLite WAL mode for concurrent access (CLI + agent).

### Potential Challenges
- Windows Service mode may face user-directory access restrictions; must run with user permissions or as a user-mode service.
- Registration for toast notifications may require additional Windows app manifest handling.
- Installer should use per-user scope to avoid UAC prompts (no admin required).
- Schema migrations must be rigorously non-destructive to avoid data loss.

---

## Milestones & Sequencing

### Project Estimate
- **Medium** — 2–4 weeks for v1.0 (core installer, frontmatter, output formats, undo). Larger/background features in v1.x.

### Team Size & Composition
- **Small team**: 1–2 engineers (1 for Rust/core features, optional 1 for installer and packaging).

### v0.1.0 — Complete ✓

All 32 GitHub issues across Epics E1–E6 shipped and closed. Core pipeline, multi-target management, Explorer context-menu integration, job inspection, retry, audit events, and CI are all live.

### Phase 1 — v1.0 Hardening (Epic [E7](https://github.com/GeziP/windows-rightclick-vault-import/issues/33), ~2 weeks)

- **Key Deliverables:**
  - `jobs undo <batch-id>` ([#34](https://github.com/GeziP/windows-rightclick-vault-import/issues/34))
  - `jobs list --json` / `--table` ([#35](https://github.com/GeziP/windows-rightclick-vault-import/issues/35))
  - YAML frontmatter injection for `.md` files ([#36](https://github.com/GeziP/windows-rightclick-vault-import/issues/36))
  - GitHub Actions release build with `.exe` artifact ([#37](https://github.com/GeziP/windows-rightclick-vault-import/issues/37))
  - NSIS one-step installer ([#38](https://github.com/GeziP/windows-rightclick-vault-import/issues/38))
  - `doctor` remediation hints ([#39](https://github.com/GeziP/windows-rightclick-vault-import/issues/39))
- **Dependencies:** v0.1.0 codebase stable, SQLite schema finalized before new migrations.

### Phase 2 — Distribution & Polish (Epic [E8](https://github.com/GeziP/windows-rightclick-vault-import/issues/40), ~1 week)

- **Key Deliverables:**
  - `vault stats` command ([#41](https://github.com/GeziP/windows-rightclick-vault-import/issues/41))
  - Windows toast notification + no console window from Explorer ([#42](https://github.com/GeziP/windows-rightclick-vault-import/issues/42))
  - winget community manifest submission ([#43](https://github.com/GeziP/windows-rightclick-vault-import/issues/43))
  - README rewrite for non-developer users ([#44](https://github.com/GeziP/windows-rightclick-vault-import/issues/44))
- **Dependencies:** v1.0 release artifact from Phase 1.

### Phase 3 — Background Service (Epic [E9](https://github.com/GeziP/windows-rightclick-vault-import/issues/45), ~2–4 weeks, v1.x)

- **Key Deliverables:**
  - Windows Service mode — `kbintake service install/start/stop/uninstall` ([#46](https://github.com/GeziP/windows-rightclick-vault-import/issues/46))
  - File-type routing rules in config.toml ([#47](https://github.com/GeziP/windows-rightclick-vault-import/issues/47))
  - SQLite schema migration versioning ([#48](https://github.com/GeziP/windows-rightclick-vault-import/issues/48))
- **Dependencies:** Phases 1 and 2 complete.
