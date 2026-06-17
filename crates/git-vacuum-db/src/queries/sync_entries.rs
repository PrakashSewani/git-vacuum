use git_vacuum_core::{NewSyncEntry, SyncEntryRow};
use rusqlite::{params, Connection, Row};

use crate::SqliteErr;

pub fn insert_many(conn: &Connection, entries: &[NewSyncEntry]) -> Result<(), SqliteErr> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            r#"INSERT INTO sync_entries
                (run_id, repo_id, operation, status, bytes_transferred, new_commits,
                 duration_ms, error_message)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )?;
        for e in entries {
            stmt.execute(params![
                e.run_id,
                e.repo_id,
                e.operation,
                e.status,
                e.bytes_transferred,
                e.new_commits,
                e.duration_ms,
                e.error_message,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn get_for_run(conn: &Connection, run_id: i64) -> Result<Vec<SyncEntryRow>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT id, run_id, repo_id, operation, status, bytes_transferred, new_commits,
                  duration_ms, error_message
           FROM sync_entries WHERE run_id = ?1 ORDER BY id"#,
    )?;
    let rows = stmt.query_map(params![run_id], row_to_entry)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn row_to_entry(row: &Row<'_>) -> rusqlite::Result<SyncEntryRow> {
    Ok(SyncEntryRow {
        id: row.get("id")?,
        run_id: row.get("run_id")?,
        repo_id: row.get("repo_id")?,
        operation: row.get("operation")?,
        status: row.get("status")?,
        bytes_transferred: row.get("bytes_transferred")?,
        new_commits: row.get("new_commits")?,
        duration_ms: row.get("duration_ms")?,
        error_message: row.get("error_message")?,
    })
}
