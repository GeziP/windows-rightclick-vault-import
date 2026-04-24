# KBIntake Install Guide

This guide is for Windows users who want KBIntake installed for Explorer right-click imports and PowerShell use.

## Recommended Install

1. Open the release page:

```text
https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0
```

2. Download `KBIntake-Setup.exe`.
3. Run the installer.
4. Open a new PowerShell window.
5. Run:

```powershell
kbintake doctor
```

Expected healthy output includes:

- `[OK] Config file`
- `[OK] Database schema`
- `[OK] Target directory`
- `[OK] Explorer context menu`
- `[OK] PATH`

## What The Installer Does

The installer is per-user and does not require Administrator privileges.

It writes files to:

```text
%LOCALAPPDATA%\Programs\kbintake
```

Installed files:

- `kbintake.exe`: command-line binary
- `kbintakew.exe`: Explorer-friendly binary that does not show a console window
- `kbintake.ico`: icon used by Explorer entries
- `Uninstall.exe`: uninstaller

It also:

- adds the install directory to the current user's `PATH`
- registers Explorer context menus for files and folders
- creates an uninstall entry in Windows Settings

## First Import

Explorer path:

1. Right-click a file or folder.
2. Choose the KBIntake action.
3. Wait for the toast notification.
4. Inspect recent jobs:

```powershell
kbintake jobs list
```

Terminal path:

```powershell
kbintake import --process C:\path\to\note.md
kbintake jobs list
```

## Optional Background Service

Most users can use Explorer imports or `import --process` without installing the service.

If you want queued imports processed continuously in the background, open an elevated Administrator PowerShell and run:

```powershell
kbintake service install
kbintake service start
kbintake service status
```

Stop and remove it later:

```powershell
kbintake service stop
kbintake service uninstall
kbintake service status
```

Expected final status:

```text
Service status: not installed
```

## Winget

The winget manifest is present in this repository under:

```text
installer\winget\1.0.0
```

It validates locally with:

```powershell
winget validate --manifest .\installer\winget\1.0.0
```

The package is not yet available from the public winget community source. After issue #43 is complete, install will use:

```powershell
winget install GeziP.KBIntake
```

## Install From Source

Use this path for development.

Install Rust from <https://rustup.rs>, then:

```powershell
cd kbintake
cargo build --release --locked --bins
```

Install the local build into your user profile:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
Copy-Item .\target\release\kbintakew.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintakew.exe" -Force
Copy-Item .\assets\kbintake.ico "$env:LOCALAPPDATA\Programs\kbintake\kbintake.ico" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor --fix
```

## Build The Installer Locally

Install NSIS, then from the repository root:

```powershell
New-Item -ItemType Directory -Force .\dist | Out-Null
Copy-Item .\kbintake\target\release\kbintake.exe .\dist\kbintake.exe -Force
Copy-Item .\kbintake\target\release\kbintakew.exe .\dist\kbintakew.exe -Force
Copy-Item .\kbintake\assets\kbintake.ico .\dist\kbintake.ico -Force
& "C:\Program Files (x86)\NSIS\makensis.exe" .\installer\kbintake.nsi
```

Output:

```text
dist\KBIntake-Setup.exe
```

## Uninstall

If installed with `KBIntake-Setup.exe`, uninstall from Windows Settings or run:

```powershell
& "$env:LOCALAPPDATA\Programs\kbintake\Uninstall.exe"
```

Manual cleanup path:

```powershell
kbintake explorer uninstall
Remove-Item "$env:LOCALAPPDATA\Programs\kbintake" -Recurse -Force
```

Runtime state is stored separately under:

```text
%LOCALAPPDATA%\kbintake
```

Do not delete it unless you intentionally want to remove config, queue history, manifests, logs, and the default vault.

## Troubleshooting

If `kbintake` is not recognized:

- open a new PowerShell window
- verify `%LOCALAPPDATA%\Programs\kbintake` is on your user `PATH`

If the Explorer menu is missing:

```powershell
kbintake explorer install
```

If KBIntake reports target problems:

```powershell
kbintake doctor
kbintake doctor --fix
```

If `kbintake service install` reports access denied:

- open PowerShell as Administrator
- run the service command again
