//! CLI registration tool for the KBIntake COM DLL.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// Re-export the CLSID so the binary doesn't need to depend on the cdylib crate name.
const CLSID_STR: &str = "A1B2C3D4-E5F6-7890-ABCD-EF1234567890";

#[derive(Parser)]
#[command(name = "kbintake-com-reg")]
#[command(about = "Register/unregister KBIntake Windows 11 Explorer COM DLL")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Register the COM DLL for Explorer integration.
    Install {
        /// Path to kbintake_com.dll
        #[arg(long)]
        dll: Option<PathBuf>,
    },
    /// Remove COM registration.
    Uninstall,
    /// Check registration status.
    Status,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Install { dll } => cmd_install(dll),
        Commands::Uninstall => cmd_uninstall(),
        Commands::Status => cmd_status(),
    }
}

#[cfg(windows)]
fn cmd_install(dll: Option<PathBuf>) {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    let dll_path = dll.unwrap_or_else(find_default_dll);
    if !dll_path.exists() {
        eprintln!("ERROR: DLL not found at {}", dll_path.display());
        std::process::exit(1);
    }

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let clsid_key = format!(r"CLSID\{{{}}}", CLSID_STR);
    let dll_str = dll_path.to_string_lossy().to_string();

    // Register CLSID.
    let (clsid_handle, _) = match hkcr.create_subkey(&clsid_key) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: failed to create CLSID key: {:#}", e);
            std::process::exit(1);
        }
    };
    let _ = clsid_handle.set_value("", &"KBIntake Explorer Command");

    let (inproc, _) = match hkcr.create_subkey(format!(r"{}\InprocServer32", clsid_key)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: failed to create InprocServer32 key: {:#}", e);
            std::process::exit(1);
        }
    };
    let _ = inproc.set_value("", &dll_str);
    let _ = inproc.set_value("ThreadingModel", &"Apartment");

    // Register ShellEx handler.
    let shell_ex_key = r"Software\Classes\*\ShellEx\ContextMenuHandlers\KBIntakeCOM";
    let (handler, _) = match hkcr.create_subkey(shell_ex_key) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: failed to create ShellEx handler key: {:#}", e);
            std::process::exit(1);
        }
    };
    let _ = handler.set_value("", &CLSID_STR);

    println!("COM DLL registered: {}", dll_path.display());
}

#[cfg(not(windows))]
fn cmd_install(_dll: Option<PathBuf>) {
    eprintln!("ERROR: COM registration is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn cmd_uninstall() {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let clsid_key = format!(r"CLSID\{{{}}}", CLSID_STR);
    let shell_ex_key = r"Software\Classes\*\ShellEx\ContextMenuHandlers\KBIntakeCOM";

    let _ = hkcr.delete_subkey_all(&clsid_key);
    let _ = hkcr.delete_subkey_all(shell_ex_key);

    println!("COM registration removed");
}

#[cfg(not(windows))]
fn cmd_uninstall() {
    eprintln!("ERROR: COM unregistration is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn cmd_status() {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let clsid_key = format!(r"CLSID\{{{}}}", CLSID_STR);
    let shell_ex_key = r"Software\Classes\*\ShellEx\ContextMenuHandlers\KBIntakeCOM";

    if let Ok(clsid_handle) = hkcr.open_subkey(&clsid_key) {
        println!("COM registration: registered");
        if let Ok(inproc) = clsid_handle.open_subkey(r"InprocServer32") {
            if let Ok(path) = inproc.get_value::<String, _>("") {
                println!("DLL path: {}", path);
            }
        }
    } else {
        println!("COM registration: not registered");
    }

    if let Ok(handler) = hkcr.open_subkey(shell_ex_key) {
        if let Ok(guid) = handler.get_value::<String, _>("") {
            println!("Explorer handler: {}", guid);
        }
    } else {
        println!("Explorer handler: not registered");
    }
}

#[cfg(not(windows))]
fn cmd_status() {
    println!("COM status: not available on this platform");
}

fn find_default_dll() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join("kbintake_com.dll");
        }
    }
    PathBuf::from("kbintake_com.dll")
}
