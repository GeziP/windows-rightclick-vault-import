pub mod schema;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};

const LATEST_SCHEMA_VERSION: i64 = 4;

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: [Migration; 4] = [
    Migration {
        version: 1,
        sql: schema::MIGRATION_001_CORE,
    },
    Migration {
        version: 2,
        sql: schema::MIGRATION_002_MANIFEST_AND_EVENTS,
    },
    Migration {
        version: 3,
        sql: schema::MIGRATION_003_EVENT_LOOKUP_INDEX,
    },
    Migration {
        version: 4,
        sql: schema::MIGRATION_004_ITEM_STORED_SHA256,
    },
];

pub fn init_schema(conn: &Connection) -> Result<()> {
    apply_pending_migrations(conn).map(|_| ())
}

pub fn apply_pending_migrations(conn: &Connection) -> Result<Vec<i64>> {
    ensure_schema_migrations_table(conn)?;
    let current = current_schema_version(conn)?;
    run_migrations(conn, &MIGRATIONS, current)
}

pub fn current_schema_version(conn: &Connection) -> Result<i64> {
    ensure_schema_migrations_table(conn)?;
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .context("failed to read schema version")
}

pub fn validate_schema(conn: &Connection) -> Result<()> {
    for table in [
        "schema_migrations",
        "batches",
        "items",
        "manifest_records",
        "events",
    ] {
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
        "idx_events_entity_created_at",
    ] {
        if sqlite_object_count(conn, "index", index)? != 1 {
            bail!("missing database index: {index}");
        }
    }

    let version = current_schema_version(conn)?;
    if version != LATEST_SCHEMA_VERSION {
        bail!("schema version out of date: {version} != {LATEST_SCHEMA_VERSION}");
    }

    Ok(())
}

pub fn latest_schema_version() -> i64 {
    LATEST_SCHEMA_VERSION
}

fn ensure_schema_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )
    .context("failed to ensure schema_migrations table")
}

fn run_migrations(conn: &Connection, migrations: &[Migration], current: i64) -> Result<Vec<i64>> {
    let mut applied = Vec::new();

    for migration in migrations
        .iter()
        .filter(|migration| migration.version > current)
    {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration.sql)
            .with_context(|| format!("failed to apply schema migration {}", migration.version))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![migration.version, Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
        applied.push(migration.version);
    }

    Ok(applied)
}

fn sqlite_object_count(conn: &Connection, kind: &str, name: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
        [kind, name],
        |row| row.get(0),
    )
    .with_context(|| format!("failed to inspect sqlite schema object {kind}:{name}"))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{
        apply_pending_migrations, current_schema_version, ensure_schema_migrations_table,
        latest_schema_version, run_migrations, sqlite_object_count, Migration,
    };

    #[test]
    fn init_schema_records_latest_version_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();

        apply_pending_migrations(&conn).unwrap();

        assert_eq!(
            current_schema_version(&conn).unwrap(),
            latest_schema_version()
        );
        assert_eq!(
            sqlite_object_count(&conn, "table", "schema_migrations").unwrap(),
            1
        );
    }

    #[test]
    fn reapplying_migrations_is_noop_when_latest() {
        let conn = Connection::open_in_memory().unwrap();
        apply_pending_migrations(&conn).unwrap();

        let applied = apply_pending_migrations(&conn).unwrap();

        assert!(applied.is_empty());
        assert_eq!(
            current_schema_version(&conn).unwrap(),
            latest_schema_version()
        );
    }

    #[test]
    fn applies_only_pending_migrations() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema_migrations_table(&conn).unwrap();
        conn.execute_batch(super::schema::MIGRATION_001_CORE)
            .unwrap();
        conn.execute_batch(super::schema::MIGRATION_002_MANIFEST_AND_EVENTS)
            .unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, '2026-04-24T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (2, '2026-04-24T00:00:01Z')",
            [],
        )
        .unwrap();

        let applied = apply_pending_migrations(&conn).unwrap();

        assert_eq!(applied, vec![3, 4]);
        assert_eq!(
            current_schema_version(&conn).unwrap(),
            latest_schema_version()
        );
        assert_eq!(
            sqlite_object_count(&conn, "index", "idx_events_entity_created_at").unwrap(),
            1
        );
    }

    #[test]
    fn failed_migration_rolls_back_without_recording_version() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema_migrations_table(&conn).unwrap();

        let result = run_migrations(
            &conn,
            &[Migration {
                version: 1,
                sql: "CREATE TABLE broken (id INTEGER PRIMARY KEY); THIS IS BAD SQL;",
            }],
            0,
        );
        assert!(result.is_err());

        assert_eq!(sqlite_object_count(&conn, "table", "broken").unwrap(), 0);
        assert_eq!(current_schema_version(&conn).unwrap(), 0);
    }
}
