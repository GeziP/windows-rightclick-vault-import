use std::path::Path;

use anyhow::{Context, Result};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "KBIntake";

#[cfg(windows)]
pub fn set_autostart(exe_path: &Path) -> Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        .with_context(|| format!("failed to open HKCU\\{RUN_KEY} for writing"))?;
    let quoted = format!("\"{}\" tray", exe_path.display());
    run.set_value(APP_NAME, &quoted)
        .with_context(|| "failed to set autostart value")?;
    Ok(())
}

#[cfg(not(windows))]
pub fn set_autostart(_exe_path: &Path) -> Result<()> {
    anyhow::bail!("autostart is only supported on Windows")
}

#[cfg(windows)]
pub fn remove_autostart() -> Result<()> {
    use std::io::ErrorKind;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        .with_context(|| format!("failed to open HKCU\\{RUN_KEY} for writing"))?;
    match run.delete_value(APP_NAME) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| "failed to remove autostart value"),
    }
}

#[cfg(not(windows))]
pub fn remove_autostart() -> Result<()> {
    anyhow::bail!("autostart is only supported on Windows")
}

#[cfg(windows)]
pub fn is_autostart_enabled() -> Result<bool> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey(RUN_KEY)
        .with_context(|| format!("failed to open HKCU\\{RUN_KEY}"))?;
    Ok(run.get_value::<String, _>(APP_NAME).is_ok())
}

#[cfg(not(windows))]
pub fn is_autostart_enabled() -> Result<bool> {
    Ok(false)
}
