use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct FrontmatterFields {
    pub source_path: String,
    pub imported_at: DateTime<Utc>,
    pub sha256: String,
    pub target: String,
}

pub fn inject_file(path: &Path, fields: &FrontmatterFields) -> Result<()> {
    let raw = std::fs::read_to_string(path)?;
    let injected = inject_text(&raw, fields);
    std::fs::write(path, injected)?;
    Ok(())
}

pub fn inject_text(content: &str, fields: &FrontmatterFields) -> String {
    let block = field_lines(fields);
    if let Some(close_index) = existing_frontmatter_close(content) {
        let close_start = close_index;
        let mut output = String::new();
        output.push_str(&content[..close_start]);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&block);
        output.push_str(&content[close_start..]);
        return output;
    }

    format!("---\n{}---\n{}", block, content)
}

pub fn file_matches_original_hash(path: &Path, expected_sha256: &str) -> Result<bool> {
    let raw = std::fs::read_to_string(path)?;
    let normalized = remove_kbintake_fields(&raw);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let actual = format!("{:x}", hasher.finalize());
    Ok(actual == expected_sha256)
}

fn remove_kbintake_fields(content: &str) -> String {
    let Some(close_index) = existing_frontmatter_close(content) else {
        return content.to_string();
    };

    let header = &content[..close_index];
    let rest = &content[close_index..];
    let Some(after_open) = header.strip_prefix("---\n") else {
        return content.to_string();
    };
    let retained = after_open
        .split_inclusive('\n')
        .filter(|line| !line.trim_start().starts_with("kbintake_"))
        .collect::<String>();

    if retained.trim().is_empty() {
        let after_close = rest
            .strip_prefix("---\r\n")
            .or_else(|| rest.strip_prefix("---\n"))
            .unwrap_or(rest);
        return after_close.to_string();
    }

    format!("---\n{}{}", retained, rest)
}

fn existing_frontmatter_close(content: &str) -> Option<usize> {
    let rest = content.strip_prefix("---\n")?;
    let mut offset = 4usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

fn field_lines(fields: &FrontmatterFields) -> String {
    format!(
        "kbintake_source: \"{}\"\nkbintake_imported_at: \"{}\"\nkbintake_sha256: \"{}\"\nkbintake_target: \"{}\"\n",
        yaml_escape(&fields.source_path),
        fields.imported_at.to_rfc3339(),
        yaml_escape(&fields.sha256),
        yaml_escape(&fields.target)
    )
}

fn yaml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn is_markdown_extension(file_ext: Option<&str>) -> bool {
    file_ext.is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::{inject_text, FrontmatterFields};

    fn fields() -> FrontmatterFields {
        FrontmatterFields {
            source_path: r#"C:\notes\source.md"#.to_string(),
            imported_at: chrono::Utc.with_ymd_and_hms(2026, 4, 23, 1, 2, 3).unwrap(),
            sha256: "abc123".to_string(),
            target: "notes".to_string(),
        }
    }

    #[test]
    fn injects_new_frontmatter_block() {
        let output = inject_text("body\n", &fields());

        assert!(output.starts_with("---\nkbintake_source: \"C:\\\\notes\\\\source.md\"\n"));
        assert!(output.contains("kbintake_sha256: \"abc123\""));
        assert!(output.ends_with("---\nbody\n"));
    }

    #[test]
    fn appends_to_existing_frontmatter() {
        let output = inject_text("---\ntitle: Note\n---\nbody\n", &fields());

        assert!(output.starts_with("---\ntitle: Note\nkbintake_source:"));
        assert!(output.contains("---\nbody\n"));
        assert_eq!(output.matches("---").count(), 2);
    }

    #[test]
    fn strips_kbintake_only_frontmatter_for_hashing() {
        let injected = inject_text("body\n", &fields());

        assert_eq!(super::remove_kbintake_fields(&injected), "body\n");
    }

    #[test]
    fn strips_kbintake_fields_from_existing_frontmatter_for_hashing() {
        let original = "---\ntitle: Note\n---\nbody\n";
        let injected = inject_text(original, &fields());

        assert_eq!(super::remove_kbintake_fields(&injected), original);
    }
}
