use std::path::Path;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ToastContent {
    pub title: String,
    pub line1: String,
    pub line2: Option<String>,
}

#[cfg(windows)]
pub fn show_toast(content: &ToastContent, icon_path: Option<&Path>) -> Result<()> {
    use winrt_notification::{IconCrop, Sound, Toast};

    let mut toast = Toast::new(Toast::POWERSHELL_APP_ID)
        .title(&content.title)
        .text1(&content.line1)
        .sound(Some(Sound::Default));
    if let Some(line2) = &content.line2 {
        toast = toast.text2(line2);
    }
    if let Some(icon_path) = icon_path.filter(|path| path.exists()) {
        toast = toast.icon(icon_path, IconCrop::Square, "KBIntake");
    }
    toast.show()?;
    Ok(())
}

#[cfg(not(windows))]
pub fn show_toast(_content: &ToastContent, _icon_path: Option<&Path>) -> Result<()> {
    Ok(())
}
