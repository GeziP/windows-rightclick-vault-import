# KBIntake v2.0 Issue Map

Last updated: 2026-04-27

## Purpose

This file is the repo-local memory for v2.0 planning alignment.

Use it to keep implementation tied to the GitHub PRD and issue trackers:

- PRD: `#53`
- Phase 1 tracker: `#54`
- Phase 2 tracker: `#55`
- Phase 3 tracker: `#56`

Do not continue v2.0 implementation from local momentum alone. Before each new slice, map the work to a specific open issue or a clearly documented acceptance slice here.

## Normalized Source Of Truth

### Product / phase level

- `#53` PRD: KBIntake v2.0 product requirements
- `#54` Phase 1 tracker
- `#55` Phase 2 tracker
- `#56` Phase 3 tracker

### Core v2 epics

- `#57` Windows 11 native context menu
- `#58` Import template system
- `#59` Target `default_subfolder`
- `#60` TUI settings
- `#61` zh-CN localization
- `#62` Watch Mode
- `#63` Obsidian URI integration
- `#64` Quick tag injection
- `#65` Vault audit
- `#66` Clipboard import and release prep
- `#67` Documentation tracker

## Known Issue Number Conflicts

The Epic bodies are not fully trustworthy as child-task references.

Examples:

- Issue `#58` uses `#64`-`#69` as template-system child tasks.
- In the live tracker set, `#64`-`#66` are Phase 3 epics and `#67`-`#73` are documentation issues.

Implication:

- Do not assume inline child issue numbers inside `#58` or nearby epics are still valid.
- When in doubt, align to the Epic acceptance criteria, the PRD, and this file.

## Current Implementation Status

### Phase 1 / Epic `#58` Import template system

Implemented on branch `v2.0`:

- v2 config sections for `templates` and `routing_rules`
- config semantic validation
- v1 `routing` compatibility retained
- template resolution with single-level `base_template`
- frontmatter merge/override
- tag merge/dedupe
- variable interpolation for 9 built-in variables
- minimal conditional rendering with `if` / `else`
- dry-run template preview
- template application during actual import
- `routing_rules.target` wired into actual import and dry-run
- route-hit visibility in dry-run, CLI output, and Explorer toast copy using the matched template name as the current rule label

Covered by tests:

- config parsing and validation
- template rendering
- dry-run preview
- end-to-end routed import into target vault/subfolder

Still open for `#58`:

- Explorer/manual-template flow and "ignore rule" escape hatch
- Watch Mode path using the same routing/template engine
- any explicit Chinese-language error/output requirements not yet implemented

### Phase 1 / Epic `#59` Target `default_subfolder`

Implemented on branch `v2.0`:

- config field on targets
- semantic validation for non-empty relative paths
- priority chain:
  - template `subfolder`
  - target `default_subfolder`
  - target root
- actual import writes to computed subfolder
- dry-run preview reflects computed subfolder

Still open for `#59`:

- confirm `doctor` surfaces target subfolder validation in the intended UX
- confirm any missing CLI/TUI editing flows

### Phase 1 / Epic `#57` Windows 11 native context menu

Not started in implementation.

Still required by Phase 1 tracker:

- feasibility spike
- go / no-go decision
- fallback documentation if moved to v2.1

## Working Rule For Future v2 Slices

Before coding:

1. Identify the governing issue.
2. Check whether the issue body contains stale child references.
3. Use the acceptance criteria in the Epic and PRD as the binding target.
4. Update this file after any meaningful slice lands.

## Recommended Next Slice

Most justified next step from the current state:

- continue Epic `#58` by exposing route-hit diagnostics in CLI/dry-run/toast surfaces

After that:

- start the `#57` Windows 11 feasibility spike, because Phase 1 acceptance still depends on a decision there
