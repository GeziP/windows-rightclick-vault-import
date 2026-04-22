use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainEvent {
    pub event_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub payload_json: Value,
    pub created_at: DateTime<Utc>,
}

impl DomainEvent {
    pub fn new(
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        event_type: impl Into<String>,
        payload_json: Value,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            event_type: event_type.into(),
            payload_json,
            created_at: Utc::now(),
        }
    }
}
