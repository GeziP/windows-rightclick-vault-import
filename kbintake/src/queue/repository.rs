use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{BatchJob, DomainEvent, ItemJob, ManifestRecord};
use crate::queue::state_machine;

pub struct Repository<'a> {
    conn: &'a Connection,
}

#[derive(Debug, Clone)]
pub struct TargetStats {
    pub imported_files: i64,
    pub storage_bytes: i64,
    pub duplicate_count: i64,
    pub success_count: i64,
    pub failed_count: i64,
    pub last_import_at: Option<String>,
}

impl<'a> Repository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert_batch(&self, batch: &BatchJob) -> Result<()> {
        self.conn.execute(
            "INSERT INTO batches (batch_id, source, target_id, status, source_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &batch.batch_id,
                &batch.source,
                &batch.target_id,
                &batch.status,
                batch.source_count,
                batch.created_at.to_rfc3339(),
                batch.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_batch_status(&self, batch_id: &str, status: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE batches SET status = ?1, updated_at = ?2 WHERE batch_id = ?3",
            params![status, Utc::now().to_rfc3339(), batch_id],
        )?;
        ensure_updated(rows, "batch", batch_id)?;
        Ok(())
    }

    pub fn refresh_batch_status(&self, batch_id: &str) -> Result<()> {
        let counts = self.conn.query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN status = ?1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = ?2 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = ?3 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = ?4 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = ?5 THEN 1 ELSE 0 END), 0)
             FROM items WHERE batch_id = ?6",
            params![
                state_machine::STATUS_QUEUED,
                state_machine::STATUS_RUNNING,
                state_machine::STATUS_SUCCESS,
                state_machine::STATUS_FAILED,
                state_machine::STATUS_DUPLICATE,
                batch_id
            ],
            |row| {
                Ok(BatchItemCounts {
                    total: row.get(0)?,
                    queued: row.get(1)?,
                    running: row.get(2)?,
                    success: row.get(3)?,
                    failed: row.get(4)?,
                    duplicate: row.get(5)?,
                })
            },
        )?;

        let status = counts.batch_status();
        self.update_batch_status(batch_id, status)
    }

    pub fn list_batches(&self, limit: i64) -> Result<Vec<BatchJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT batch_id, source, target_id, status, source_count, created_at, updated_at
             FROM batches ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_batch)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_batches_filtered(&self, limit: i64, status: Option<&str>) -> Result<Vec<BatchJob>> {
        if let Some(status) = status {
            let mut stmt = self.conn.prepare(
                "SELECT batch_id, source, target_id, status, source_count, created_at, updated_at
                 FROM batches WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![status, limit], row_to_batch)?;
            return rows
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into);
        }

        self.list_batches(limit)
    }

    pub fn get_batch(&self, batch_id: &str) -> Result<BatchJob> {
        self.conn
            .query_row(
                "SELECT batch_id, source, target_id, status, source_count, created_at, updated_at
                 FROM batches WHERE batch_id = ?1",
                params![batch_id],
                row_to_batch,
            )
            .map_err(Into::into)
    }

    pub fn insert_item(&self, item: &ItemJob) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items (
                item_id, batch_id, target_id, source_path, source_name, file_ext, status, stage,
                source_size, sha256, stored_sha256, stored_path, duplicate_of, error_code, error_message,
                cli_tags, import_subfolder, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                &item.item_id,
                &item.batch_id,
                &item.target_id,
                &item.source_path,
                &item.source_name,
                item.file_ext.as_deref(),
                &item.status,
                item.stage.as_deref(),
                item.source_size,
                item.sha256.as_deref(),
                item.stored_sha256.as_deref(),
                item.stored_path.as_deref(),
                item.duplicate_of.as_deref(),
                item.error_code.as_deref(),
                item.error_message.as_deref(),
                item.cli_tags.as_deref(),
                item.import_subfolder.as_deref(),
                item.created_at.to_rfc3339(),
                item.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_items_by_batch(&self, batch_id: &str) -> Result<Vec<ItemJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, batch_id, target_id, source_path, source_name, file_ext, status, stage,
                    source_size, sha256, stored_sha256, stored_path, duplicate_of, error_code, error_message,
                    cli_tags, import_subfolder, created_at, updated_at
             FROM items WHERE batch_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![batch_id], row_to_item)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn count_queued_items_by_target(&self, target_id: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM items WHERE target_id = ?1 AND status = ?2",
                params![target_id, state_machine::STATUS_QUEUED],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn target_stats(&self, target_id: &str) -> Result<TargetStats> {
        self.conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM manifest_records WHERE target_id = ?1),
                    (SELECT COALESCE(SUM(COALESCE(source_size, 0)), 0) FROM manifest_records WHERE target_id = ?1),
                    (SELECT COUNT(*) FROM items WHERE target_id = ?1 AND status = ?2),
                    (SELECT COUNT(*) FROM items WHERE target_id = ?1 AND status = ?3),
                    (SELECT COUNT(*) FROM items WHERE target_id = ?1 AND status = ?4),
                    (SELECT MAX(e.created_at)
                     FROM events e
                     JOIN items i ON i.item_id = e.entity_id
                     WHERE e.entity_type = 'item' AND e.event_type = 'item.success' AND i.target_id = ?1)",
                params![
                    target_id,
                    state_machine::STATUS_DUPLICATE,
                    state_machine::STATUS_SUCCESS,
                    state_machine::STATUS_FAILED
                ],
                |row| {
                    Ok(TargetStats {
                        imported_files: row.get(0)?,
                        storage_bytes: row.get(1)?,
                        duplicate_count: row.get(2)?,
                        success_count: row.get(3)?,
                        failed_count: row.get(4)?,
                        last_import_at: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn next_queued_item(&self) -> Result<Option<ItemJob>> {
        self.conn
            .query_row(
                "SELECT item_id, batch_id, target_id, source_path, source_name, file_ext, status, stage,
                        source_size, sha256, stored_sha256, stored_path, duplicate_of, error_code, error_message,
                        cli_tags, import_subfolder, created_at, updated_at
                 FROM items WHERE status = ?1 ORDER BY created_at ASC LIMIT 1",
                params![state_machine::STATUS_QUEUED],
                row_to_item,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn update_item_running(&self, item_id: &str, stage: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items SET status = ?1, stage = ?2, updated_at = ?3 WHERE item_id = ?4",
            params![
                state_machine::STATUS_RUNNING,
                stage,
                Utc::now().to_rfc3339(),
                item_id
            ],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn update_item_hash(&self, item_id: &str, sha256: &str, source_size: i64) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items SET sha256 = ?1, source_size = ?2, updated_at = ?3 WHERE item_id = ?4",
            params![sha256, source_size, Utc::now().to_rfc3339(), item_id],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn mark_item_success(
        &self,
        item_id: &str,
        stored_path: &str,
        stored_sha256: &str,
    ) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items SET status = ?1, stored_path = ?2, stored_sha256 = ?3, stage = NULL, updated_at = ?4 WHERE item_id = ?5",
            params![
                state_machine::STATUS_SUCCESS,
                stored_path,
                stored_sha256,
                Utc::now().to_rfc3339(),
                item_id
            ],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn mark_item_duplicate(&self, item_id: &str, duplicate_of: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items SET status = ?1, duplicate_of = ?2, stage = NULL, updated_at = ?3 WHERE item_id = ?4",
            params![state_machine::STATUS_DUPLICATE, duplicate_of, Utc::now().to_rfc3339(), item_id],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn mark_item_failed(
        &self,
        item_id: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items SET status = ?1, error_code = ?2, error_message = ?3, stage = NULL, updated_at = ?4 WHERE item_id = ?5",
            params![state_machine::STATUS_FAILED, error_code, error_message, Utc::now().to_rfc3339(), item_id],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn mark_item_undone(&self, item_id: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items
             SET status = ?1, stage = NULL, error_code = NULL, error_message = NULL, updated_at = ?2
             WHERE item_id = ?3",
            params![
                state_machine::STATUS_UNDONE,
                Utc::now().to_rfc3339(),
                item_id
            ],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn mark_item_undo_skipped_modified(&self, item_id: &str, message: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items
             SET status = ?1, stage = NULL, error_code = ?2, error_message = ?3, updated_at = ?4
             WHERE item_id = ?5",
            params![
                state_machine::STATUS_UNDO_SKIPPED_MODIFIED,
                "E_UNDO_MODIFIED",
                message,
                Utc::now().to_rfc3339(),
                item_id
            ],
        )?;
        ensure_updated(rows, "item", item_id)?;
        Ok(())
    }

    pub fn retry_failed_items_by_batch(&self, batch_id: &str) -> Result<usize> {
        self.get_batch(batch_id)?;
        let rows = self.conn.execute(
            "UPDATE items
             SET status = ?1, stage = NULL, error_code = NULL, error_message = NULL, updated_at = ?2
             WHERE batch_id = ?3 AND status = ?4",
            params![
                state_machine::STATUS_QUEUED,
                Utc::now().to_rfc3339(),
                batch_id,
                state_machine::STATUS_FAILED
            ],
        )?;
        self.refresh_batch_status(batch_id)?;
        Ok(rows)
    }

    pub fn insert_manifest(&self, record: &ManifestRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO manifest_records (
                record_id, item_id, target_id, source_path, stored_path, source_name,
                file_ext, source_size, sha256, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &record.record_id,
                &record.item_id,
                &record.target_id,
                &record.source_path,
                &record.stored_path,
                &record.source_name,
                record.file_ext.as_deref(),
                record.source_size,
                &record.sha256,
                record.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn delete_manifest_by_item(&self, item_id: &str) -> Result<usize> {
        self.conn
            .execute(
                "DELETE FROM manifest_records WHERE item_id = ?1",
                params![item_id],
            )
            .map_err(Into::into)
    }

    pub fn find_manifest_by_hash(&self, target_id: &str, sha256: &str) -> Result<Option<(String, String)>> {
        self.conn
            .query_row(
                "SELECT record_id, stored_path FROM manifest_records WHERE target_id = ?1 AND sha256 = ?2",
                params![target_id, sha256],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_manifests_by_target(
        &self,
        target_id: &str,
    ) -> Result<Vec<crate::processor::audit::ManifestEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT record_id, stored_path, source_name, sha256 FROM manifest_records WHERE target_id = ?1",
        )?;
        let rows = stmt
            .query_map(params![target_id], |row| {
                Ok(crate::processor::audit::ManifestEntry {
                    record_id: row.get(0)?,
                    stored_path: row.get(1)?,
                    source_name: row.get(2)?,
                    sha256: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn mark_manifest_missing(&self, record_id: &str) -> Result<()> {
        let rows = self.conn.execute(
            "DELETE FROM manifest_records WHERE record_id = ?1",
            params![record_id],
        )?;
        if rows == 0 {
            return Err(anyhow::anyhow!("record not found: {record_id}"));
        }
        Ok(())
    }

    pub fn mark_manifest_duplicate(&self, record_id: &str) -> Result<()> {
        let rows = self.conn.execute(
            "DELETE FROM manifest_records WHERE record_id = ?1",
            params![record_id],
        )?;
        if rows == 0 {
            return Err(anyhow::anyhow!("record not found: {record_id}"));
        }
        Ok(())
    }

    pub fn insert_event(&self, event: &DomainEvent) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events (
                event_id, entity_type, entity_id, event_type, payload_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &event.event_id,
                &event.entity_type,
                &event.entity_id,
                &event.event_type,
                event.payload_json.to_string(),
                event.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_events_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<DomainEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT event_id, entity_type, entity_id, event_type, payload_json, created_at
             FROM events WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![entity_type, entity_id], row_to_event)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

fn ensure_updated(rows: usize, entity: &str, id: &str) -> Result<()> {
    if rows == 0 {
        anyhow::bail!("{entity} not found: {id}");
    }
    Ok(())
}

struct BatchItemCounts {
    total: i64,
    queued: i64,
    running: i64,
    success: i64,
    failed: i64,
    duplicate: i64,
}

impl BatchItemCounts {
    fn batch_status(&self) -> &'static str {
        let terminal = self.success + self.failed + self.duplicate;
        if self.running > 0 || (self.queued > 0 && terminal > 0) {
            return state_machine::STATUS_RUNNING;
        }
        if self.queued > 0 {
            return state_machine::STATUS_QUEUED;
        }
        if self.total == 0 || self.failed > 0 {
            return state_machine::STATUS_FAILED;
        }
        if self.success > 0 {
            return state_machine::STATUS_SUCCESS;
        }
        state_machine::STATUS_DUPLICATE
    }
}

fn parse_utc(value: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })
}

fn row_to_batch(row: &rusqlite::Row<'_>) -> rusqlite::Result<BatchJob> {
    Ok(BatchJob {
        batch_id: row.get(0)?,
        source: row.get(1)?,
        target_id: row.get(2)?,
        status: row.get(3)?,
        source_count: row.get(4)?,
        created_at: parse_utc(row.get(5)?)?,
        updated_at: parse_utc(row.get(6)?)?,
    })
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemJob> {
    Ok(ItemJob {
        item_id: row.get(0)?,
        batch_id: row.get(1)?,
        target_id: row.get(2)?,
        source_path: row.get(3)?,
        source_name: row.get(4)?,
        file_ext: row.get(5)?,
        status: row.get(6)?,
        stage: row.get(7)?,
        source_size: row.get(8)?,
        sha256: row.get(9)?,
        stored_sha256: row.get(10)?,
        stored_path: row.get(11)?,
        duplicate_of: row.get(12)?,
        error_code: row.get(13)?,
        error_message: row.get(14)?,
        cli_tags: row.get(15)?,
        import_subfolder: row.get(16)?,
        created_at: parse_utc(row.get(17)?)?,
        updated_at: parse_utc(row.get(18)?)?,
    })
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<DomainEvent> {
    let payload_raw: String = row.get(4)?;
    let payload_json = serde_json::from_str(&payload_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(DomainEvent {
        event_id: row.get(0)?,
        entity_type: row.get(1)?,
        entity_id: row.get(2)?,
        event_type: row.get(3)?,
        payload_json,
        created_at: parse_utc(row.get(5)?)?,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::Connection;

    use super::Repository;
    use crate::db;
    use crate::domain::{BatchJob, DomainEvent, ItemJob, ManifestRecord};
    use crate::queue::state_machine;

    fn repo_with_conn() -> (Connection, BatchJob, ItemJob) {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let batch = BatchJob::new("test", "default", 1);
        let item = ItemJob::new(
            batch.batch_id.clone(),
            batch.target_id.clone(),
            PathBuf::from("source.md"),
        );
        {
            let repo = Repository::new(&conn);
            repo.insert_batch(&batch).unwrap();
            repo.insert_item(&item).unwrap();
        }
        (conn, batch, item)
    }

    #[test]
    fn update_methods_reject_missing_rows() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let repo = Repository::new(&conn);

        assert!(repo.update_batch_status("missing", "running").is_err());
        assert!(repo.update_item_running("missing", "hashing").is_err());
        assert!(repo.update_item_hash("missing", "abc", 3).is_err());
        assert!(repo
            .mark_item_success("missing", "stored.md", "stored-hash")
            .is_err());
        assert!(repo.mark_item_duplicate("missing", "record").is_err());
        assert!(repo
            .mark_item_failed("missing", "E_TEST", "failed")
            .is_err());
        assert!(repo.mark_item_undone("missing").is_err());
        assert!(repo
            .mark_item_undo_skipped_modified("missing", "modified")
            .is_err());
    }

    #[test]
    fn refresh_batch_status_marks_success_when_all_items_succeed() {
        let (conn, batch, item) = repo_with_conn();
        let repo = Repository::new(&conn);

        repo.mark_item_success(&item.item_id, "vault/source.md", "stored-hash")
            .unwrap();
        repo.refresh_batch_status(&batch.batch_id).unwrap();

        assert_eq!(
            repo.get_batch(&batch.batch_id).unwrap().status,
            state_machine::STATUS_SUCCESS
        );
    }

    #[test]
    fn refresh_batch_status_marks_failed_when_any_item_fails() {
        let (conn, batch, item) = repo_with_conn();
        let repo = Repository::new(&conn);
        let second = ItemJob::new(
            batch.batch_id.clone(),
            batch.target_id.clone(),
            PathBuf::from("other.md"),
        );
        repo.insert_item(&second).unwrap();

        repo.mark_item_success(&item.item_id, "vault/source.md", "stored-hash")
            .unwrap();
        repo.mark_item_failed(&second.item_id, "E_TEST", "failed")
            .unwrap();
        repo.refresh_batch_status(&batch.batch_id).unwrap();

        assert_eq!(
            repo.get_batch(&batch.batch_id).unwrap().status,
            state_machine::STATUS_FAILED
        );
    }

    #[test]
    fn find_manifest_by_hash_returns_existing_record() {
        let (conn, _batch, item) = repo_with_conn();
        let repo = Repository::new(&conn);
        let record = ManifestRecord::new(
            item.item_id,
            item.target_id,
            item.source_path,
            "vault/source.md".to_string(),
            item.source_name,
            item.file_ext,
            Some(3),
            "abc123".to_string(),
        );
        let record_id = record.record_id.clone();

        repo.insert_manifest(&record).unwrap();

        assert_eq!(
            repo.find_manifest_by_hash("default", "abc123")
                .unwrap()
                .map(|(id, _)| id),
            Some(record_id)
        );
    }

    #[test]
    fn retry_failed_items_by_batch_resets_only_failed_items() {
        let (conn, batch, item) = repo_with_conn();
        let repo = Repository::new(&conn);
        let second = ItemJob::new(
            batch.batch_id.clone(),
            batch.target_id.clone(),
            PathBuf::from("other.md"),
        );
        repo.insert_item(&second).unwrap();
        repo.mark_item_failed(&item.item_id, "E_TEST", "failed")
            .unwrap();
        repo.mark_item_success(&second.item_id, "vault/other.md", "stored-hash")
            .unwrap();

        let retried = repo.retry_failed_items_by_batch(&batch.batch_id).unwrap();

        let items = repo.list_items_by_batch(&batch.batch_id).unwrap();
        assert_eq!(retried, 1);
        assert_eq!(items[0].status, state_machine::STATUS_QUEUED);
        assert_eq!(items[0].error_code, None);
        assert_eq!(items[0].error_message, None);
        assert_eq!(items[1].status, state_machine::STATUS_SUCCESS);
        assert_eq!(
            repo.get_batch(&batch.batch_id).unwrap().status,
            state_machine::STATUS_RUNNING
        );
    }

    #[test]
    fn retry_failed_items_by_batch_rejects_missing_batch() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let repo = Repository::new(&conn);

        let err = repo.retry_failed_items_by_batch("missing").unwrap_err();

        assert!(err.to_string().contains("Query returned no rows"));
    }

    #[test]
    fn inserts_and_lists_events_for_entity() {
        let (conn, _batch, item) = repo_with_conn();
        let repo = Repository::new(&conn);
        let event = DomainEvent::new(
            "item",
            item.item_id.clone(),
            "item.failed",
            serde_json::json!({
                "error_code": "E_TEST"
            }),
        );

        repo.insert_event(&event).unwrap();

        let events = repo.list_events_for_entity("item", &item.item_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "item.failed");
        assert_eq!(events[0].payload_json["error_code"], "E_TEST");
    }

    #[test]
    fn list_batches_filtered_returns_matching_status_only() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let repo = Repository::new(&conn);
        let queued_batch = BatchJob::new("test", "default", 1);
        let failed_batch = BatchJob::new("test", "default", 2);
        repo.insert_batch(&queued_batch).unwrap();
        repo.insert_batch(&failed_batch).unwrap();
        repo.update_batch_status(&failed_batch.batch_id, state_machine::STATUS_FAILED)
            .unwrap();

        let only_failed = repo
            .list_batches_filtered(20, Some(state_machine::STATUS_FAILED))
            .unwrap();
        let only_queued = repo
            .list_batches_filtered(20, Some(state_machine::STATUS_QUEUED))
            .unwrap();

        assert_eq!(only_failed.len(), 1);
        assert_eq!(only_failed[0].batch_id, failed_batch.batch_id);
        assert_eq!(only_queued.len(), 1);
        assert_eq!(only_queued[0].batch_id, queued_batch.batch_id);
    }

    #[test]
    fn delete_manifest_by_item_removes_only_matching_rows() {
        let (conn, _batch, item) = repo_with_conn();
        let repo = Repository::new(&conn);
        let first = ManifestRecord::new(
            item.item_id.clone(),
            item.target_id.clone(),
            item.source_path.clone(),
            "vault/source.md".to_string(),
            item.source_name.clone(),
            item.file_ext.clone(),
            Some(3),
            "hash-1".to_string(),
        );
        let second = ManifestRecord::new(
            "other-item".to_string(),
            item.target_id.clone(),
            "other.md".to_string(),
            "vault/other.md".to_string(),
            "other.md".to_string(),
            Some("md".to_string()),
            Some(5),
            "hash-2".to_string(),
        );
        repo.insert_manifest(&first).unwrap();
        repo.insert_manifest(&second).unwrap();

        let removed = repo.delete_manifest_by_item(&item.item_id).unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM manifest_records", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(removed, 1);
        assert_eq!(remaining, 1);
    }
}
