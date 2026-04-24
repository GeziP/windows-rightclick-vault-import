# KBIntake Install Guide

## Recommended Install

Use this path if you want KBIntake available from Explorer and PowerShell without building Rust code yourself.

1. Open the project's GitHub Releases page.
2. Download `KBIntake-Setup.exe`.
3. Run the installer.
4. Right-click a file in Explorer and choose the KBIntake action.

The installer places KBIntake in:

```text
%LOCALAPPDATA%\Programs\kbintake
```

It also:

- installs `kbintake.exe`
- installs `kbintakew.exe`
- installs `kbintake.ico`
- adds the install directory to your user PATH
- registers Explorer context-menu entries
- writes an uninstall entry in Windows settings

## First Run Check

Open PowerShell and run:

```powershell
kbintake doctor --fix
```

That creates any missing local directories and reports whether the database, target directory, Explorer menu, and PATH look healthy.

## First Import

Explorer path:

1. Right-click a file.
2. Choose the KBIntake action.
3. Open PowerShell and run:

```powershell
kbintake jobs list
```

Terminal path:

```powershell
kbintake import --process C:\path\to\note.md
kbintake jobs list
```

## Optional Background Service

If you want queued imports processed automatically in the background, open an elevated Administrator PowerShell and run:

```powershell
kbintake service install
kbintake service start
kbintake service status
```

To stop and remove it later:

```powershell
kbintake service stop
kbintake service uninstall
```

## Install From Source

If you are developing KBIntake or want a local build:

```powershell
cd kbintake
cargo build --release
```

Copy it into a stable per-user location:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
Copy-Item .\target\release\kbintakew.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintakew.exe" -Force
Copy-Item .\assets\kbintake.ico "$env:LOCALAPPDATA\Programs\kbintake\kbintake.ico" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor --fix
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" explorer install
```

For repeatable development validation, use the scripts from the repository root:

```powershell
.\scripts\validate-explorer-toast.ps1
.\scripts\validate-service-mode.ps1
```

The service validation script installs a Windows Service and must be run from an elevated Administrator PowerShell session.

## Uninstall

If KBIntake was installed through the installer, remove it from Windows Settings like any other app.

Manual cleanup path:

```powershell
kbintake explorer uninstall
Remove-Item "$env:LOCALAPPDATA\Programs\kbintake" -Recurse -Force
```

Runtime state is stored separately under `%LOCALAPPDATA%\kbintake`. Leave that directory alone unless you intentionally want to remove config, queue history, manifests, and the default vault.

## Troubleshooting

If `kbintake` is not recognized:

- open a new PowerShell window
- or add `%LOCALAPPDATA%\Programs\kbintake` to your user PATH manually

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
- then run the service command again
