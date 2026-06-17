use std::path::Path;

use rusqlite::Connection;

use crate::SqliteErr;

pub fn open_connection(path: &Path) -> Result<Connection, SqliteErr> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SqliteErr(rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))
            })?;
        }
    }

    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection, SqliteErr> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), SqliteErr> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000i64)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}
