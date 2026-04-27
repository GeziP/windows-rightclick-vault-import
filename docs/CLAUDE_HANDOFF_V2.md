# KBIntake v2.0 Handoff Notes

Last updated: 2026-04-27

## Purpose

This file is the current implementation and planning handoff for continuing KBIntake v2.0 development in another agent session.

Use this together with:

- `docs/PRD.md`
- `docs/V2_DEVELOPMENT_PLAN.md`
- `docs/V2_ISSUE_MAP.md`
- GitHub issues `#53`, `#54`, `#57`, `#58`, `#59`

## Current Branch State

- active branch: `v2.0`
- working tree status at handoff: clean

Recent v2 commits on this branch:

- `192b34b` Add Windows 11 COM feasibility probe
- `cc98c8d` Expose matched routing rules in previews and toasts
- `64bb55d` Align v2 work with issue tracking
- `36ca41b` Route v2 imports to configured targets
- `ea5203e` Apply templates during import
- `11c1e8e` Add template conditional rendering

## Source Of Truth

Do not continue from local code momentum alone.

For v2.0 work, use:

- product requirements: issue `#53` and `docs/PRD.md`
- phase tracking: issues `#54`, `#55`, `#56`
- normalized repo-local mapping: `docs/V2_ISSUE_MAP.md`

Important constraint:

- Some older Epic bodies contain stale child-issue references.
- In particular, issue `#58` references child numbers that now overlap with later documentation issues.
- Treat `docs/V2_ISSUE_MAP.md` as the normalization layer before picking the next slice.

## What Is Implemented

### Phase 1 / `#58` Import template system

Implemented:

- `templates` config section
- `routing_rules` config section
- v1 `routing` compatibility retained
- `kbintake config validate`
- `base_template` single-level inheritance
- frontmatter merge/override
- tag merge/dedupe
- 9 built-in interpolation variables:
  - `file_name`
  - `file_ext`
  - `file_size_kb`
  - `imported_at`
  - `imported_at_date`
  - `source_path`
  - `sha256`
  - `target_name`
  - `batch_id`
- conditional rendering:
  - `{{#if}}`
  - `{{#else}}`
  - `== != > >= < <= contains && ||`
- dry-run template preview
- template application during actual import
- `routing_rules.target` wired into real import and dry-run
- route-hit visibility:
  - dry-run table shows `Rule`
  - dry-run JSON shows `matched_rule_template`
  - CLI import output can print `Routing rule: ...`
  - Explorer toast text includes rule context

Current implementation detail:

- the current “rule label” surfaced to users is the matched `template` name
- there is no separate route name field in schema yet

### Phase 1 / `#59` Target `default_subfolder`

Implemented:

- target `default_subfolder`
- semantic validation for relative/non-empty values
- subfolder priority chain:
  - template `subfolder`
  - target `default_subfolder`
  - target root
- actual import writes into computed subfolder
- dry-run reflects computed destination

### Phase 1 / `#57` Windows 11 native context menu

Feasibility work started, but COM implementation is **not** started.

Implemented:

- hidden command:
  - `kbintake explorer com-feasibility`
- probe module:
  - `kbintake/src/explorer/com_probe.rs`
- spike report:
  - `docs/WIN11_COM_FEASIBILITY.md`

Current architectural verdict:

- native Windows 11 `IExplorerCommand` work requires a separate in-proc COM DLL spike
- do **not** try to evolve the current exe-only registry registration directly into native Win11 integration

## Open Gaps

### Still open in `#58`

- Explorer/manual-template flow
- “ignore rule, choose template manually” escape hatch
- Watch Mode using the same routing/template engine
- explicit zh-CN user-facing output requirements

### Still open in `#59`

- confirm `doctor` behavior is sufficient for target subfolder validation UX
- no TUI/settings flow exists yet for editing these fields

### Still open in `#57`

- separate DLL proof of concept
- static Explorer command registration on Windows 11
- invoke KBIntake from that DLL path
- real install/uninstall validation on Windows 11
- go/no-go decision for v2.0 vs v2.1

## Recommended Next Step

The most justified next slice is:

### Continue `#57` with a separate DLL proof of concept

Target outcome:

1. create a Windows-only COM DLL spike crate
2. make it compile in CI or local Windows build
3. expose one static `IExplorerCommand`
4. prove registration/unregistration
5. invoke the existing KBIntake path from the COM command

Do **not** start with dynamic template submenu rendering.
Do **not** merge COM into the main `kbintake.exe` / `kbintakew.exe` registration path yet.

If the DLL spike proves unstable, move native Windows 11 menu work to v2.1 and keep the current registry integration.

## Validation State At Handoff

Most recent successful validations:

For template/routing work:

```powershell
cd kbintake
cargo fmt --all
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

For the COM feasibility probe slice:

```powershell
cd kbintake
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Reason the second slice did not use `--locked`:

- it introduced the `windows` crate dependency
- `Cargo.lock` was refreshed accordingly

## Files Most Relevant To Continue From

- `docs/V2_ISSUE_MAP.md`
- `docs/WIN11_COM_FEASIBILITY.md`
- `docs/V2_DEVELOPMENT_PLAN.md`
- `kbintake/src/config/mod.rs`
- `kbintake/src/processor/template.rs`
- `kbintake/src/processor/dry_run.rs`
- `kbintake/src/cli/mod.rs`
- `kbintake/src/explorer/mod.rs`
- `kbintake/src/explorer/com_probe.rs`
- `kbintake/tests/mvp_flow.rs`

## Handoff Guidance

Before writing the next v2.0 code:

1. read `docs/V2_ISSUE_MAP.md`
2. pick the governing issue
3. verify the issue body does not contain stale child references
4. implement a narrow slice
5. update `docs/V2_ISSUE_MAP.md` after the slice lands
