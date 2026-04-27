# v1.0 Release Checklist

Use this checklist for the Windows `v1.0.0` release and future patch releases.

## Release Metadata

- Version: `v1.0.0`
- Release commit: `f6cec70` for the tag build, followed by `9cab26d` for winget manifest hash update
- GitHub Release: `https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0`
- Validation date: 2026-04-24

## Automated Checks

Run from `kbintake/`:

```powershell
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked
cargo build --release --locked --bins
```

The release workflow also runs tests and builds release binaries for `x86_64-pc-windows-msvc`.

## Release Assets

Expected GitHub Release assets:

- `KBIntake-Setup.exe`
- `kbintake.exe`
- `kbintakew.exe`
- `kbintake.ico`
- `SHA256SUMS.txt`

Users should prefer `KBIntake-Setup.exe`.

## Installer Smoke Check

Install from the generated setup executable:

```powershell
.\dist\KBIntake-Setup.exe
```

Open a new PowerShell window:

```powershell
kbintake --version
kbintake version
kbintake doctor
```

Expected result:

- version commands print `kbintake 1.0.0`
- config is under `%LOCALAPPDATA%\kbintake`
- schema reports `Schema version: 3 (up to date)`
- target directory is OK
- Explorer context menu is registered
- PATH check is OK

## Explorer Toast Smoke Check

Run from the repository root:

```powershell
.\scripts\validate-explorer-toast.ps1
```

Expected result:

- `kbintake.exe`, `kbintakew.exe`, and `kbintake.ico` are staged under `%LOCALAPPDATA%\Programs\kbintake`
- Explorer registry entries point at `kbintakew.exe explorer run-import`
- manual prompts confirm success, duplicate, and failure toasts
- manual prompts confirm Explorer imports do not show a console window

## Service Mode Smoke Check

Run from an elevated Administrator PowerShell session:

```powershell
.\scripts\validate-service-mode.ps1
```

Expected result:

- `KBIntake` service installs, starts, stops, and uninstalls
- a queued import is processed automatically by the service
- `%TEMP%\kbintake-service-check\logs\service.log*` is created
- post-uninstall status is `not installed`

Manual item still pending:

- reboot-resume validation for service mode

## Winget Manifest Check

The winget installer manifest declares `Microsoft.VCRedist.2015+.x64` because the
MSVC release binaries require `VCRUNTIME140.dll` on clean Windows installs.

Run from the repository root:

```powershell
winget validate --manifest .\installer\winget\1.0.0
```

Expected result:

```text
Manifest validation succeeded.
```

Local manifest install requires this setting to be enabled by an administrator:

```powershell
winget settings --enable LocalManifestFiles
```

Then test:

```powershell
winget install --manifest .\installer\winget\1.0.0 --silent --accept-source-agreements --accept-package-agreements
```

## Rollback

Remove Explorer entries:

```powershell
kbintake explorer uninstall
```

Remove the Windows Service if installed:

```powershell
kbintake service stop
kbintake service uninstall
```

Uninstall KBIntake from Windows Settings or run:

```powershell
& "$env:LOCALAPPDATA\Programs\kbintake\Uninstall.exe"
```

Runtime state is stored under `%LOCALAPPDATA%\kbintake`. Do not delete it unless you intentionally want to remove config, queue history, manifests, logs, and the default vault.

## Known Limitations

- Release binaries are not Authenticode signed yet.
- Windows SmartScreen may warn on first run.
- Public winget availability is not complete until `microsoft/winget-pkgs#364698` is merged.
- Only local-folder targets are implemented.
- Windows service reboot-resume validation is still manual.
