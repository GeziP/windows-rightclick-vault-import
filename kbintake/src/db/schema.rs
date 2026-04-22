pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS batches (
    batch_id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    target_id TEXT NOT NULL,
    status TEXT NOT NULL,
    source_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS items (
    item_id TEXT PRIMARY KEY,
    batch_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    source_path TEXT NOT NULL,
    source_name TEXT NOT NULL,
    file_ext TEXT,
    status TEXT NOT NULL,
    stage TEXT,
    source_size INTEGER,
    sha256 TEXT,
    stored_path TEXT,
    duplicate_of TEXT,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(batch_id) REFERENCES batches(batch_id)
);

CREATE TABLE IF NOT EXISTS manifest_records (
    record_id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    source_path TEXT NOT NULL,
    stored_path TEXT NOT NULL,
    source_name TEXT NOT NULL,
    file_ext TEXT,
    source_size INTEGER,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_manifest_target_hash
ON manifest_records(target_id, sha256);

CREATE INDEX IF NOT EXISTS idx_batches_created_at
ON batches(created_at);

CREATE INDEX IF NOT EXISTS idx_items_batch
ON items(batch_id);

CREATE INDEX IF NOT EXISTS idx_items_status_created_at
ON items(status, created_at);

CREATE INDEX IF NOT EXISTS idx_items_target_hash
ON items(target_id, sha256);

CREATE TABLE IF NOT EXISTS events (
    event_id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#;
