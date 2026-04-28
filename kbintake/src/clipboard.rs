use std::path::PathBuf;

use anyhow::{bail, Result};

pub fn read_clipboard_paths() -> Result<Vec<PathBuf>> {
    let text = read_clipboard_text()?;
    if text.trim().is_empty() {
        bail!("clipboard is empty or does not contain text");
    }
    let paths: Vec<PathBuf> = text
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let path = PathBuf::from(line);
            if path.exists() {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    if paths.is_empty() {
        bail!("clipboard contains no valid file paths");
    }
    Ok(paths)
}

#[cfg(windows)]
fn read_clipboard_text() -> Result<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};

    unsafe {
        if OpenClipboard(HWND::default()).is_err() {
            bail!("failed to open clipboard");
        }

        let result = (|| -> Result<String> {
            let handle = GetClipboardData(13u32)?; // CF_UNICODETEXT = 13
            if handle.is_invalid() {
                bail!("clipboard does not contain text");
            }
            let ptr = handle.0 as *const u16;
            let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
            let os_str = OsString::from_wide(std::slice::from_raw_parts(ptr, len));
            Ok(os_str.to_string_lossy().into_owned())
        })();

        let _ = CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
fn read_clipboard_text() -> Result<String> {
    bail!("clipboard import is only supported on Windows");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_paths_from_text() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("a.txt");
        let file2 = dir.path().join("b.md");
        std::fs::write(&file1, "a").unwrap();
        std::fs::write(&file2, "b").unwrap();
        let text = format!("{}\n{}\nnot_a_path", file1.display(), file2.display());
        let paths: Vec<PathBuf> = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .filter_map(|l| {
                let p = PathBuf::from(l);
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(paths.len(), 2);
    }
}
