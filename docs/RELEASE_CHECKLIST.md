# v0.1 Release Checklist

Use this checklist for the first local Windows MVP release.

## Release Metadata

- Version:
- Release commit:
- Windows version:
- Rust version from `rustc --version`:
- Cargo version from `cargo --version`:
- Validation date:

## Automated Checks

Run from `kbintake/`:

```powershell
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked
cargo build --release --locked
```

Record the command output or CI run link in the release notes.

## CLI Validation

Use a temporary test directory that is not an important vault:

```powershell
$sample = New-Item -ItemType Directory -Force "$env:TEMP\kbintake-release-sample"
Set-Content "$sample\note.md" "release smoke test"
.\target\release\kbintake.exe doctor
.\target\release\kbintake.exe config-show
.\target\release\kbintake.exe import "$sample\note.md"
.\target\release\kbintake.exe jobs list
.\target\release\kbintake.exe agent
.\target\release\kbintake.exe jobs list
.\target\release\kbintake.exe import --process "$sample\note.md"
```

Expected result:

- `doctor --fix` prints config, database, default target, and `Schema version: 3 (up to date)`.
- `import` prints a batch ID, item count, and target.
- `agent` reports at least one processed item.
- `jobs list` shows the batch moving from queued/running to success.
- `import --process` queues and processes work in one command.
- The copied file exists under the configured target vault.

## Explorer Registration Validation

Install the release executable to a stable per-user path:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
Copy-Item .\assets\kbintake.ico "$env:LOCALAPPDATA\Programs\kbintake\kbintake.ico" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor
```

Register and verify:

```powershell
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" explorer install
reg query "HKCU\Software\Classes\*\shell\KBIntake\command"
reg query "HKCU\Software\Classes\Directory\shell\KBIntake\command"
reg query "HKCU\Software\Classes\*\shell\KBIntake" /v Icon
reg query "HKCU\Software\Classes\Directory\shell\KBIntake" /v Icon
```

Manual Explorer smoke test:

- Right-click a regular file and choose the KBIntake action.
- Right-click a directory and choose the KBIntake action.
- Run `kbintake.exe jobs list` to confirm the right-click imports completed or queued as expected.

## Rollback

Remove context-menu entries:

```powershell
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" explorer uninstall
```

Verify the keys are gone:

```powershell
reg query "HKCU\Software\Classes\*\shell\KBIntake"
reg query "HKCU\Software\Classes\Directory\shell\KBIntake"
```

Remove the installed executable if needed:

```powershell
Remove-Item "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
```

Local runtime state is stored under `%LOCALAPPDATA%\kbintake` by default. Do not delete it unless you intentionally want to remove local config, queue state, manifests, and the default vault.

## Known Limitations

- Registry scripts still use editable placeholders when used manually; prefer `kbintake explorer install`.
- Only a local-folder target is implemented.
- Windows service mode still needs elevated SCM validation and reboot-resume validation.
- Explorer validation is manual.
