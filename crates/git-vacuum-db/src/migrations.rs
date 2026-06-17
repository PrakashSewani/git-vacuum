use rusqlite::Connection;

use crate::SqliteErr;

const SCHEMA: &str = include_str!("../migrations/001_initial_schema.sql");

pub fn run(conn: &Connection) -> Result<(), SqliteErr> {
    conn.execute_batch(SCHEMA)?;
    record_version(conn, 1)?;
    Ok(())
}

fn record_version(conn: &Connection, v: i64) -> Result<(), SqliteErr> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (?1)",
        rusqlite::params![v],
    )?;
    Ok(())
}
