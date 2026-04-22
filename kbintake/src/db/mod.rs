pub mod schema;

use anyhow::Result;
use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(schema::SCHEMA)?;
    Ok(())
}
