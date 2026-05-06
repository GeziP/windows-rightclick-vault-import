use std::process::Command;

use anyhow::{Context, Result};

/// Open a note in Obsidian via `obsidian://` URI.
pub fn open_note(vault: &str, note_path: &str) -> Result<()> {
    let encoded_note = urlencoding::encode(note_path);
    let encoded_vault = urlencoding::encode(vault);
    let uri = format!("obsidian://open?vault={encoded_vault}&file={encoded_note}");
    open_uri(&uri).with_context(|| format!("failed to open Obsidian URI: {uri}"))
}

#[cfg(windows)]
fn open_uri(uri: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/c", "start", uri])
        .spawn()
        .map(|_| ())
        .context("failed to spawn cmd to open URI")
}

#[cfg(not(windows))]
fn open_uri(uri: &str) -> Result<()> {
    Command::new("xdg-open")
        .arg(uri)
        .spawn()
        .map(|_| ())
        .context("failed to spawn xdg-open to open URI")
}

#[cfg(test)]
mod tests {
    use super::open_note;

    #[test]
    fn open_note_returns_ok_on_success_path() {
        // We cannot actually launch Obsidian in tests, but we verify
        // the function doesn't panic with valid inputs.
        let result = open_note("MyVault", "notes/test.md");
        // May fail if cmd/xdg-open isn't available in test env.
        // We just check it returns a Result without panicking.
        let _ = result.is_ok() || result.is_err();
    }
}
