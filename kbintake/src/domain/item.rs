use std::path::PathBuf;

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ItemJob {
    pub item_id: String,
    pub batch_id: String,
    pub target_id: String,
    pub source_path: String,
    pub source_name: String,
    pub file_ext: Option<String>,
    pub status: String,
    pub stage: Option<String>,
    pub source_size: Option<i64>,
    pub sha256: Option<String>,
    pub stored_path: Option<String>,
    pub duplicate_of: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ItemJob {
    pub fn new(batch_id: String, target_id: String, source_path: PathBuf) -> Self {
        let now = Utc::now();
        let source_name = source_path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let file_ext = source_path
            .extension()
            .map(|v| v.to_string_lossy().to_string());

        Self {
            item_id: Uuid::new_v4().to_string(),
            batch_id,
            target_id,
            source_path: source_path.to_string_lossy().to_string(),
            source_name,
            file_ext,
            status: "queued".to_string(),
            stage: None,
            source_size: None,
            sha256: None,
            stored_path: None,
            duplicate_of: None,
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }
}
