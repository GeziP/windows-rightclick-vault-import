use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BatchJob {
    pub batch_id: String,
    pub source: String,
    pub target_id: String,
    pub status: String,
    pub source_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BatchJob {
    pub fn new(source: impl Into<String>, target_id: impl Into<String>, source_count: i64) -> Self {
        let now = Utc::now();
        Self {
            batch_id: Uuid::new_v4().to_string(),
            source: source.into(),
            target_id: target_id.into(),
            status: "queued".to_string(),
            source_count,
            created_at: now,
            updated_at: now,
        }
    }
}
