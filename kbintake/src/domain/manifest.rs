use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ManifestRecord {
    pub record_id: String,
    pub item_id: String,
    pub target_id: String,
    pub source_path: String,
    pub stored_path: String,
    pub source_name: String,
    pub file_ext: Option<String>,
    pub source_size: Option<i64>,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
}

impl ManifestRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        item_id: String,
        target_id: String,
        source_path: String,
        stored_path: String,
        source_name: String,
        file_ext: Option<String>,
        source_size: Option<i64>,
        sha256: String,
    ) -> Self {
        Self {
            record_id: Uuid::new_v4().to_string(),
            item_id,
            target_id,
            source_path,
            stored_path,
            source_name,
            file_ext,
            source_size,
            sha256,
            created_at: Utc::now(),
        }
    }
}
