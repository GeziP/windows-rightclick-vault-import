//! COM registration / unregistration helpers.

#[cfg(windows)]
use winreg::enums::HKEY_CLASSES_ROOT;
#[cfg(windows)]
use winreg::RegKey;

use crate::command::CLSID_KBINTAKE_COMMAND;

/// Registry key for the COM CLSID.
fn clsid_key() -> String {
    format!(r"CLSID\{{{}}}", guid_to_string(&CLSID_KBINTAKE_COMMAND))
}

/// Registry key for the top-level Explorer verb (Win11 native context menu).
fn shell_verb_key() -> String {
    r"*\shell\KBIntake".to_string()
}

/// GUID to registry-friendly string.
pub fn guid_to_string(guid: &windows::core::GUID) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

#[cfg(windows)]
pub fn register(dll_path: &std::path::Path) -> anyhow::Result<()> {
    let dll_str = dll_path.to_string_lossy().to_string();
    let clsid = clsid_key();

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let (clsid_handle, _) = hkcr.create_subkey(&clsid)?;
    clsid_handle.set_value("", &"KBIntake Explorer Command")?;

    let (inproc, _) = hkcr.create_subkey(format!(r"{}\InprocServer32", clsid))?;
    inproc.set_value("", &dll_str)?;
    inproc.set_value("ThreadingModel", &"Apartment")?;

    // Register as a top-level verb for Win11 native context menu.
    // Using ExplorerCommandHandler binds the verb to our IExplorerCommand COM object.
    let verb_key = shell_verb_key();
    let (verb, _) = hkcr.create_subkey(&verb_key)?;
    verb.set_value("", &"Add to Knowledge Base")?;
    verb.set_value(
        "ExplorerCommandHandler",
        &guid_to_string(&CLSID_KBINTAKE_COMMAND),
    )?;

    // Also add an icon reference.
    let (verb_command, _) = hkcr.create_subkey(format!(r"{}\command", verb_key))?;
    verb_command.set_value("", &format!("\"{}\" import --process \"%1\"", dll_str))?;

    Ok(())
}

#[cfg(windows)]
pub fn unregister() -> anyhow::Result<()> {
    use std::io::ErrorKind;

    let clsid = clsid_key();
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);

    if let Err(e) = hkcr.delete_subkey_all(&clsid) {
        if e.kind() != ErrorKind::NotFound {
            return Err(e.into());
        }
    }
    if let Err(e) = hkcr.delete_subkey_all(shell_verb_key()) {
        if e.kind() != ErrorKind::NotFound {
            return Err(e.into());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn register(_dll_path: &std::path::Path) -> anyhow::Result<()> {
    anyhow::bail!("COM registration is only supported on Windows")
}

#[cfg(not(windows))]
pub fn unregister() -> anyhow::Result<()> {
    anyhow::bail!("COM unregistration is only supported on Windows")
}
