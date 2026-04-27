# Windows 11 COM Feasibility Spike

Last updated: 2026-04-27

Related issues:

- PRD: `#53`
- Phase 1 tracker: `#54`
- Epic: `#57`

## Purpose

This spike answers one narrow question:

Can KBIntake move from the current HKCU shell-command registration model to a native Windows 11 first-level Explorer menu using `IExplorerCommand` without destabilizing the installer and release model?

## Current State

Today KBIntake uses:

- HKCU `Software\\Classes\\*\\shell\\KBIntake`
- HKCU `Software\\Classes\\Directory\\shell\\KBIntake`
- command target: `kbintakew.exe explorer run-import "%1"`

This works on Windows 10 and on Windows 11 under "Show more options", but it is not a native first-level Windows 11 menu integration.

## Spike Findings

### 1. Native Windows 11 menu support changes the packaging model

`IExplorerCommand` is an in-process COM extension model.

Practical implication:

- the current exe-only registration path is not enough
- a separate Windows-only COM DLL artifact is required for a serious spike
- installer logic must register and unregister COM, not only write shell command keys

### 2. The main Rust binary should not absorb the COM implementation

The current codebase has a clean separation:

- import engine and queue live in the Rust crate
- Explorer integration is currently a thin launch path into `kbintakew.exe`

That separation should stay.

Practical implication:

- keep the import engine in the existing crate
- keep COM in a separate DLL-oriented spike target
- invoke the existing executable or library surface from the COM layer

### 3. The right first spike is installation and invocation, not dynamic menus

Before implementing template-driven submenus, the project first needs to prove:

- a COM DLL can be built on CI
- install and uninstall are clean on Windows 11
- Explorer can instantiate the command without visible latency regressions
- one static command can invoke the existing KBIntake import path

Only after that should dynamic submenu rendering be attempted.

## Repo Support Added In This Slice

The repository now includes a hidden probe command:

```powershell
kbintake explorer com-feasibility
```

It currently verifies:

- the host can initialize a COM apartment
- the project records the packaging verdict for the current architecture decision

This is not the final COM implementation. It is a repeatable checkpoint for the spike.

## Verdict

Current decision:

- **Proceed only with a separate DLL spike**
- **Do not embed COM into the current exe registration model**

That means the next implementation slice for `#57` should create a minimal Windows-only COM DLL proof of concept that:

1. registers successfully
2. exposes one static Explorer command
3. invokes KBIntake
4. uninstalls cleanly

## Go / No-Go Gate

Keep `#57` in v2.0 only if the DLL spike proves all of the following quickly:

- registration works on a clean Windows 11 machine
- uninstall leaves no broken Explorer state
- command invocation latency is acceptable
- installer complexity remains manageable

Otherwise:

- keep the current registry-based Explorer flow
- move native Windows 11 first-level menu work to v2.1
