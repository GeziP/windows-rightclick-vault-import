pub mod schema;

use anyhow::{bail, Context, Result};
use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(schema::SCHEMA)
        .context("failed to execute database schema")?;
    Ok(())
}

pub fn validate_schema(conn: &Connection) -> Result<()> {
    for table in ["batches", "items", "manifest_records", "events"] {
        if sqlite_object_count(conn, "table", table)? != 1 {
            bail!("missing database table: {table}");
        }
    }

    for index in [
        "idx_manifest_target_hash",
        "idx_batches_created_at",
        "idx_items_batch",
        "idx_items_status_created_at",
        "idx_items_target_hash",
    ] {
        if sqlite_object_count(conn, "index", index)? != 1 {
            bail!("missing database index: {index}");
        }
    }

    Ok(())
}

fn sqlite_object_count(conn: &Connection, kind: &str, name: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
        [kind, name],
        |row| row.get(0),
    )
    .with_context(|| format!("failed to inspect sqlite schema object {kind}:{name}"))
}
