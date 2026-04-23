# windows-rightclick-vault-import

Windows right-click vault import tool planning repository.

## Development

Install the Rust toolchain, then run commands from `kbintake/`:

```powershell
cargo build
cargo run -- doctor
cargo run -- config show
cargo run -- config set-target <vault-path>
cargo run -- import <path>
cargo run -- agent
cargo run -- jobs list
```

## Windows Explorer Integration

Build a release executable and copy it to a stable per-user path before registering context-menu entries:

```powershell
cd kbintake
cargo build --release
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\kbintake"
Copy-Item .\target\release\kbintake.exe "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" -Force
& "$env:LOCALAPPDATA\Programs\kbintake\kbintake.exe" doctor
```

Before applying registry files, review them and replace the placeholder executable path with the expanded absolute path for your account, for example:

```text
C:\Users\<user>\AppData\Local\Programs\kbintake\kbintake.exe
```

The registration scripts should only modify these per-user keys:

```text
HKEY_CURRENT_USER\Software\Classes\*\shell\KBIntake
HKEY_CURRENT_USER\Software\Classes\Directory\shell\KBIntake
```

Register the file and directory context menus:

```powershell
reg import .\kbintake\scripts\register_file_context_menu.reg
reg import .\kbintake\scripts\register_dir_context_menu.reg
```

Verify the registered commands:

```powershell
reg query "HKCU\Software\Classes\*\shell\KBIntake\command"
reg query "HKCU\Software\Classes\Directory\shell\KBIntake\command"
```

To roll back the context-menu entries:

```powershell
reg import .\kbintake\scripts\unregister_file_context_menu.reg
reg import .\kbintake\scripts\unregister_dir_context_menu.reg
```

After unregistering, the `reg query` commands above should report that the keys do not exist. Do not apply registry files until the executable path has been checked and the file contents have been reviewed.

## Project Documents

- [Contributor guide](AGENTS.md)
- [Development roadmap](docs/ROADMAP.md)
- [v0.1 release checklist](docs/RELEASE_CHECKLIST.md)
- [Rust scaffold](kbintake_rust_scaffold.md)
