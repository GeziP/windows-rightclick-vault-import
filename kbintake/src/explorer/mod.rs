pub mod com_probe;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const FILE_MENU_KEY: &str = r"Software\Classes\*\shell\KBIntake";
pub const FILE_COMMAND_KEY: &str = r"Software\Classes\*\shell\KBIntake\command";
pub const DIR_MENU_KEY: &str = r"Software\Classes\Directory\shell\KBIntake";
pub const DIR_COMMAND_KEY: &str = r"Software\Classes\Directory\shell\KBIntake\command";

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub exe_path: PathBuf,
    pub icon_path: Option<PathBuf>,
    pub process: bool,
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRegistration {
    pub menu_key: &'static str,
    pub command_key: &'static str,
    pub title: String,
    pub command: String,
    pub icon_path: Option<PathBuf>,
}

pub fn default_install_options(queue_only: bool, lang: &str) -> Result<InstallOptions> {
    let current_exe =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let exe_path = discover_gui_exe_next_to_exe(&current_exe).unwrap_or(current_exe);
    let icon_path = discover_icon_next_to_exe(&exe_path);
    Ok(InstallOptions {
        exe_path,
        icon_path,
        process: !queue_only,
        lang: lang.to_string(),
    })
}

pub fn build_registrations(options: &InstallOptions) -> Vec<MenuRegistration> {
    let command = build_import_command(&options.exe_path, options.process);
    let file_title = crate::i18n::tr("explorer.menu_file", &options.lang);
    let dir_title = crate::i18n::tr("explorer.menu_dir", &options.lang);
    vec![
        MenuRegistration {
            menu_key: FILE_MENU_KEY,
            command_key: FILE_COMMAND_KEY,
            title: file_title,
            command: command.clone(),
            icon_path: options.icon_path.clone(),
        },
        MenuRegistration {
            menu_key: DIR_MENU_KEY,
            command_key: DIR_COMMAND_KEY,
            title: dir_title,
            command,
            icon_path: options.icon_path.clone(),
        },
    ]
}

pub fn build_import_command(exe_path: &Path, process: bool) -> String {
    let queue_only_arg = if process { "" } else { " --queue-only" };
    format!(
        "\"{}\" explorer run-import{queue_only_arg} \"%1\"",
        escape_command_path(exe_path)
    )
}

pub fn discover_icon_next_to_exe(exe_path: &Path) -> Option<PathBuf> {
    let icon_path = exe_path.with_file_name("kbintake.ico");
    icon_path.exists().then_some(icon_path)
}

pub fn discover_gui_exe_next_to_exe(exe_path: &Path) -> Option<PathBuf> {
    let gui_path = gui_exe_path_next_to_exe(exe_path);
    gui_path.exists().then_some(gui_path)
}

pub fn gui_exe_path_next_to_exe(exe_path: &Path) -> PathBuf {
    exe_path.with_file_name("kbintakew.exe")
}

fn escape_command_path(path: &Path) -> String {
    path.display().to_string().replace('"', "\\\"")
}

#[cfg(windows)]
pub fn is_installed() -> Result<bool> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    Ok(hkcu.open_subkey(FILE_COMMAND_KEY).is_ok() && hkcu.open_subkey(DIR_COMMAND_KEY).is_ok())
}

#[cfg(not(windows))]
pub fn is_installed() -> Result<bool> {
    Ok(false)
}

#[cfg(windows)]
pub fn install(options: &InstallOptions) -> Result<Vec<MenuRegistration>> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let registrations = build_registrations(options);

    for registration in &registrations {
        let (menu_key, _) = hkcu
            .create_subkey(registration.menu_key)
            .with_context(|| format!("failed to create HKCU\\{}", registration.menu_key))?;
        menu_key
            .set_value("", &registration.title)
            .with_context(|| format!("failed to set title for HKCU\\{}", registration.menu_key))?;
        if let Some(icon_path) = &registration.icon_path {
            let icon = icon_path.display().to_string();
            menu_key.set_value("Icon", &icon).with_context(|| {
                format!("failed to set icon for HKCU\\{}", registration.menu_key)
            })?;
        } else {
            let _ = menu_key.delete_value("Icon");
        }

        let (command_key, _) = hkcu
            .create_subkey(registration.command_key)
            .with_context(|| format!("failed to create HKCU\\{}", registration.command_key))?;
        command_key
            .set_value("", &registration.command)
            .with_context(|| {
                format!(
                    "failed to set command for HKCU\\{}",
                    registration.command_key
                )
            })?;
    }

    Ok(registrations)
}

#[cfg(not(windows))]
pub fn install(_options: &InstallOptions) -> Result<Vec<MenuRegistration>> {
    anyhow::bail!("Explorer context-menu installation is only supported on Windows")
}

#[cfg(windows)]
pub fn uninstall() -> Result<()> {
    use std::io::ErrorKind;
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for key in [FILE_MENU_KEY, DIR_MENU_KEY] {
        match hkcu.delete_subkey_all(key) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to delete HKCU\\{key}"));
            }
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn uninstall() -> Result<()> {
    anyhow::bail!("Explorer context-menu uninstallation is only supported on Windows")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_import_command, build_registrations, gui_exe_path_next_to_exe, InstallOptions,
        DIR_MENU_KEY, FILE_MENU_KEY,
    };

    #[test]
    fn import_command_processes_by_default() {
        let command = build_import_command(&PathBuf::from(r"C:\Tools\kbintakew.exe"), true);

        assert_eq!(
            command,
            r#""C:\Tools\kbintakew.exe" explorer run-import "%1""#
        );
    }

    #[test]
    fn import_command_can_queue_only() {
        let command = build_import_command(&PathBuf::from(r"C:\Tools\kbintakew.exe"), false);

        assert_eq!(
            command,
            r#""C:\Tools\kbintakew.exe" explorer run-import --queue-only "%1""#
        );
    }

    #[test]
    fn registrations_cover_file_and_directory_menus() {
        let options = InstallOptions {
            exe_path: PathBuf::from(r"C:\Tools\kbintake.exe"),
            icon_path: Some(PathBuf::from(r"C:\Tools\kbintake.ico")),
            process: true,
            lang: "en".to_string(),
        };

        let registrations = build_registrations(&options);

        assert_eq!(registrations.len(), 2);
        assert_eq!(registrations[0].menu_key, FILE_MENU_KEY);
        assert_eq!(registrations[1].menu_key, DIR_MENU_KEY);
        assert!(registrations
            .iter()
            .all(|registration| registration.icon_path == options.icon_path));
    }

    #[test]
    fn gui_exe_lookup_switches_to_kbintakew_name() {
        let path = gui_exe_path_next_to_exe(&PathBuf::from(r"C:\Tools\kbintake.exe"));

        assert_eq!(path, PathBuf::from(r"C:\Tools\kbintakew.exe"));
    }
}
