use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::domain::Target;
use crate::processor::frontmatter::is_markdown_extension;
use crate::queue::repository::Repository;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditReport {
    pub target_name: String,
    pub orphan_files: Vec<String>,
    pub missing_files: Vec<ManifestEntry>,
    pub duplicate_records: Vec<DuplicateGroup>,
    pub malformed_frontmatter: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestEntry {
    pub record_id: String,
    pub stored_path: String,
    pub source_name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    pub sha256: String,
    pub records: Vec<ManifestEntry>,
}

pub struct AuditFixResult {
    pub cleaned_missing: usize,
    pub deduplicated: usize,
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\")
}

pub fn audit_vault(target: &Target, repo: &Repository<'_>) -> Result<AuditReport> {
    let root = &target.root_path;

    // Collect all files in vault directory
    let vault_files: HashSet<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| normalize_path(&e.path().to_string_lossy()))
        .collect();

    // Collect all manifest records for this target
    let manifest_entries = repo.list_manifests_by_target(&target.target_id)?;

    // Build set of stored paths from manifest (normalized)
    let manifest_paths: HashSet<String> = manifest_entries
        .iter()
        .map(|e| normalize_path(&e.stored_path))
        .collect();

    // Orphan files: in vault but not in manifest
    let orphan_files = vault_files
        .iter()
        .filter(|p| !manifest_paths.contains(p.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    // Missing files: in manifest but file doesn't exist
    let missing_files = manifest_entries
        .iter()
        .filter(|e| !Path::new(&e.stored_path).exists())
        .cloned()
        .collect::<Vec<_>>();

    // Duplicate records: same SHA-256, multiple records
    let mut hash_groups: HashMap<String, Vec<ManifestEntry>> = HashMap::new();
    for entry in &manifest_entries {
        hash_groups
            .entry(entry.sha256.clone())
            .or_default()
            .push(entry.clone());
    }
    let duplicate_records = hash_groups
        .into_iter()
        .filter(|(_, entries)| entries.len() > 1)
        .map(|(sha256, records)| DuplicateGroup { sha256, records })
        .collect::<Vec<_>>();

    // Malformed frontmatter: .md files missing kbintake_ fields
    let malformed_frontmatter = vault_files
        .iter()
        .filter(|p| {
            let ext = Path::new(p).extension().and_then(|e| e.to_str());
            is_markdown_extension(ext)
        })
        .filter(|p| !has_kbintake_frontmatter(p))
        .cloned()
        .collect::<Vec<_>>();

    Ok(AuditReport {
        target_name: target.name.clone(),
        orphan_files,
        missing_files,
        duplicate_records,
        malformed_frontmatter,
    })
}

pub fn fix_audit_issues(
    _target: &Target,
    repo: &Repository<'_>,
    report: &AuditReport,
) -> Result<AuditFixResult> {
    let mut cleaned_missing = 0;
    let mut deduplicated = 0;

    // Mark manifest records for missing files
    for entry in &report.missing_files {
        if let Ok(()) = repo.mark_manifest_missing(&entry.record_id) {
            cleaned_missing += 1;
        }
    }

    // Deduplicate: keep newest, mark older records
    for group in &report.duplicate_records {
        let mut entries = group.records.clone();
        entries.sort_by(|a, b| a.record_id.cmp(&b.record_id));
        // Skip the last (newest) entry, mark the rest
        for entry in entries.iter().take(entries.len().saturating_sub(1)) {
            if let Ok(()) = repo.mark_manifest_duplicate(&entry.record_id) {
                deduplicated += 1;
            }
        }
    }

    Ok(AuditFixResult {
        cleaned_missing,
        deduplicated,
    })
}

fn has_kbintake_frontmatter(path: &str) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    if !content.starts_with("---") {
        return false;
    }
    content.contains("kbintake_source:") || content.contains("kbintake_sha256:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_kbintake_frontmatter_detects_injected_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(
            &file,
            "---\nkbintake_source: \"C:\\test.md\"\nkbintake_sha256: \"abc\"\n---\nbody\n",
        )
        .unwrap();
        assert!(has_kbintake_frontmatter(&file.to_string_lossy()));
    }

    #[test]
    fn has_kbintake_frontmatter_rejects_plain_md() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(&file, "# Hello\nbody text\n").unwrap();
        assert!(!has_kbintake_frontmatter(&file.to_string_lossy()));
    }

    #[test]
    fn has_kbintake_frontmatter_rejects_frontmatter_without_kbintake() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(&file, "---\ntitle: Test\n---\nbody\n").unwrap();
        assert!(!has_kbintake_frontmatter(&file.to_string_lossy()));
    }
}
