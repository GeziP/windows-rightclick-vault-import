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

    // Register as a top-level verb for Win11 native context menu.
    let verb_key = r"*\shell\KBIntake";
    let (verb, _) = match hkcr.create_subkey(verb_key) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: failed to create shell verb key: {:#}", e);
            std::process::exit(1);
        }
    };
    let _ = verb.set_value("", &"Add to Knowledge Base");
    let _ = verb.set_value("ExplorerCommandHandler", &CLSID_STR);

    let (verb_cmd, _) = match hkcr.create_subkey(format!(r"{}\command", verb_key)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ERROR: failed to create verb command key: {:#}", e);
            std::process::exit(1);
        }
    };
    let _ = verb_cmd.set_value("", &format!("\"{}\" import --process \"%1\"", dll_str));

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
    let verb_key = r"*\shell\KBIntake";

    let _ = hkcr.delete_subkey_all(&clsid_key);
    let _ = hkcr.delete_subkey_all(verb_key);

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
    let verb_key = r"*\shell\KBIntake";

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

    if let Ok(verb) = hkcr.open_subkey(verb_key) {
        if let Ok(label) = verb.get_value::<String, _>("") {
            println!("Explorer verb: {}", label);
        }
        if let Ok(handler) = verb.get_value::<String, _>("ExplorerCommandHandler") {
            println!("ExplorerCommandHandler: {}", handler);
        }
    } else {
        println!("Explorer verb: not registered");
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
