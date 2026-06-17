use rusqlite::{params, Connection, OptionalExtension};

use crate::SqliteErr;

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, SqliteErr> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let v: Option<String> = stmt.query_row(params![key], |r| r.get(0)).optional()?;
    Ok(v)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), SqliteErr> {
    conn.execute(
        r#"INSERT INTO settings (key, value) VALUES (?1, ?2)
           ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')"#,
        params![key, value],
    )?;
    Ok(())
}

pub fn get_all(conn: &Connection) -> Result<Vec<(String, String)>, SqliteErr> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
