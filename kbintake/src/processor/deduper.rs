use anyhow::Result;

use crate::queue::repository::Repository;

pub fn find_duplicate_record(
    repo: &Repository<'_>,
    target_id: &str,
    sha256: &str,
) -> Result<Option<String>> {
    repo.find_manifest_by_hash(target_id, sha256)
}
