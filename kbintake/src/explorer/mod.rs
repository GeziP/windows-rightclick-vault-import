pub mod com_probe;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const FILE_MENU_KEY: &str = r"Software\Classes\*\shell\KBIntake";
pub const DIR_MENU_KEY: &str = r"Software\Classes\Directory\shell\KBIntake";

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub exe_path: PathBuf,
    pub icon_path: Option<PathBuf>,
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadingMenu {
    pub menu_key: &'static str,
    pub title: String,
    pub icon_path: Option<PathBuf>,
    pub sub_items: Vec<SubMenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubMenuItem {
    pub sub_key: &'static str,
    pub label: String,
    pub command: String,
}

pub fn default_install_options(lang: &str) -> Result<InstallOptions> {
    let current_exe =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let exe_path = discover_gui_exe_next_to_exe(&current_exe).unwrap_or(current_exe);
    let icon_path = discover_icon_next_to_exe(&exe_path);
    Ok(InstallOptions {
        exe_path,
        icon_path,
        lang: lang.to_string(),
    })
}

pub fn build_cascading_registrations(options: &InstallOptions) -> Vec<CascadingMenu> {
    let title = crate::i18n::tr("explorer.menu_title", &options.lang);
    let sub_items = build_sub_items(&options.exe_path, &options.lang);
    vec![
        CascadingMenu {
            menu_key: FILE_MENU_KEY,
            title: title.clone(),
            icon_path: options.icon_path.clone(),
            sub_items: sub_items.clone(),
        },
        CascadingMenu {
            menu_key: DIR_MENU_KEY,
            title,
            icon_path: options.icon_path.clone(),
            sub_items,
        },
    ]
}

fn build_sub_items(exe_path: &Path, lang: &str) -> Vec<SubMenuItem> {
    let escaped = escape_command_path(exe_path);
    vec![
        SubMenuItem {
            sub_key: "01import",
            label: crate::i18n::tr("explorer.sub_import", lang),
            command: format!(r#""{escaped}" explorer run-import "%1""#),
        },
        SubMenuItem {
            sub_key: "02queue",
            label: crate::i18n::tr("explorer.sub_queue", lang),
            command: format!(r#""{escaped}" explorer run-import --queue-only "%1""#),
        },
        SubMenuItem {
            sub_key: "03settings",
            label: crate::i18n::tr("explorer.sub_settings", lang),
            command: format!(r#""{escaped}" explorer settings"#),
        },
    ]
}

pub fn build_import_command(exe_path: &Path, process: bool) -> String {
    let queue_only_arg = if process { "" } else { " --queue-only" };
    format!(
        r#""{}" explorer run-import{queue_only_arg} "%1""#,
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
    Ok(hkcu.open_subkey(FILE_MENU_KEY).is_ok() && hkcu.open_subkey(DIR_MENU_KEY).is_ok())
}

#[cfg(not(windows))]
pub fn is_installed() -> Result<bool> {
    Ok(false)
}

#[cfg(windows)]
pub fn install(options: &InstallOptions) -> Result<Vec<CascadingMenu>> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let menus = build_cascading_registrations(options);

    for menu in &menus {
        let (menu_key, _) = hkcu
            .create_subkey(menu.menu_key)
            .with_context(|| format!("failed to create HKCU\\{}", menu.menu_key))?;
        menu_key
            .set_value("MUIVerb", &menu.title)
            .with_context(|| format!("failed to set MUIVerb for HKCU\\{}", menu.menu_key))?;
        menu_key
            .set_value("subcommands", &"")
            .with_context(|| format!("failed to set subcommands for HKCU\\{}", menu.menu_key))?;
        if let Some(icon_path) = &menu.icon_path {
            let icon = icon_path.display().to_string();
            menu_key
                .set_value("Icon", &icon)
                .with_context(|| format!("failed to set icon for HKCU\\{}", menu.menu_key))?;
        } else {
            let _ = menu_key.delete_value("Icon");
        }

        for sub in &menu.sub_items {
            let sub_key_path = format!("{}\\shell\\{}", menu.menu_key, sub.sub_key);
            let (sub_key, _) = hkcu
                .create_subkey(&sub_key_path)
                .with_context(|| format!("failed to create HKCU\\{}", sub_key_path))?;
            sub_key
                .set_value("MUIVerb", &sub.label)
                .with_context(|| format!("failed to set MUIVerb for HKCU\\{}", sub_key_path))?;

            let cmd_key_path = format!("{}\\command", sub_key_path);
            let (cmd_key, _) = hkcu
                .create_subkey(&cmd_key_path)
                .with_context(|| format!("failed to create HKCU\\{}", cmd_key_path))?;
            cmd_key
                .set_value("", &sub.command)
                .with_context(|| format!("failed to set command for HKCU\\{}", cmd_key_path))?;
        }
    }

    Ok(menus)
}

#[cfg(not(windows))]
pub fn install(_options: &InstallOptions) -> Result<Vec<CascadingMenu>> {
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
        build_cascading_registrations, build_import_command, InstallOptions, DIR_MENU_KEY,
        FILE_MENU_KEY,
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
    fn cascading_registrations_cover_file_and_directory_with_three_sub_items() {
        let options = InstallOptions {
            exe_path: PathBuf::from(r"C:\Tools\kbintake.exe"),
            icon_path: Some(PathBuf::from(r"C:\Tools\kbintake.ico")),
            lang: "en".to_string(),
        };

        let menus = build_cascading_registrations(&options);

        assert_eq!(menus.len(), 2);
        assert_eq!(menus[0].menu_key, FILE_MENU_KEY);
        assert_eq!(menus[1].menu_key, DIR_MENU_KEY);
        assert!(menus.iter().all(|menu| menu.icon_path == options.icon_path));

        for menu in &menus {
            assert_eq!(menu.sub_items.len(), 3);
            assert_eq!(menu.sub_items[0].sub_key, "01import");
            assert_eq!(menu.sub_items[1].sub_key, "02queue");
            assert_eq!(menu.sub_items[2].sub_key, "03settings");
        }
    }

    #[test]
    fn sub_items_have_correct_command_format() {
        let options = InstallOptions {
            exe_path: PathBuf::from(r"C:\Tools\kbintakew.exe"),
            icon_path: None,
            lang: "en".to_string(),
        };

        let menus = build_cascading_registrations(&options);
        let file_menu = &menus[0];

        assert!(file_menu.sub_items[0].command.contains("run-import \"%1\""));
        assert!(
            file_menu.sub_items[0].command.contains("run-import \"%1\"")
                && !file_menu.sub_items[0].command.contains("--queue-only")
        );
        assert!(file_menu.sub_items[1]
            .command
            .contains("run-import --queue-only \"%1\""));
        assert!(file_menu.sub_items[2].command.contains("explorer settings"));
    }

    #[test]
    fn gui_exe_path_resolves_to_kbintakew_name() {
        let path = super::gui_exe_path_next_to_exe(&PathBuf::from(r"C:\Tools\kbintake.exe"));

        assert_eq!(path, PathBuf::from(r"C:\Tools\kbintakew.exe"));
    }
}
