use chrono::{DateTime, Utc};
use git_vacuum_core::{NewSyncRun, SyncRunRow, SyncRunUpdate};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::SqliteErr;

pub fn insert(conn: &Connection, run: &NewSyncRun) -> Result<i64, SqliteErr> {
    conn.execute(
        r#"INSERT INTO sync_runs (started_at, trigger, total_repos, options_json, status)
           VALUES (?1, ?2, ?3, ?4, 'running')"#,
        params![
            run.started_at.to_rfc3339(),
            run.trigger,
            run.total_repos,
            run.options_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, u: &SyncRunUpdate) -> Result<(), SqliteErr> {
    if let Some(s) = &u.status {
        conn.execute(
            "UPDATE sync_runs SET status = ?1 WHERE id = ?2",
            params![s, id],
        )?;
    }
    if let Some(d) = u.completed_at {
        conn.execute(
            "UPDATE sync_runs SET completed_at = ?1 WHERE id = ?2",
            params![d.to_rfc3339(), id],
        )?;
    }
    if let Some(n) = u.cloned_count {
        conn.execute(
            "UPDATE sync_runs SET cloned_count = ?1 WHERE id = ?2",
            params![n, id],
        )?;
    }
    if let Some(n) = u.updated_count {
        conn.execute(
            "UPDATE sync_runs SET updated_count = ?1 WHERE id = ?2",
            params![n, id],
        )?;
    }
    if let Some(n) = u.failed_count {
        conn.execute(
            "UPDATE sync_runs SET failed_count = ?1 WHERE id = ?2",
            params![n, id],
        )?;
    }
    if let Some(n) = u.bytes_transferred {
        conn.execute(
            "UPDATE sync_runs SET bytes_transferred = ?1 WHERE id = ?2",
            params![n, id],
        )?;
    }
    Ok(())
}

pub fn get_many(conn: &Connection, limit: usize) -> Result<Vec<SyncRunRow>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT id, started_at, completed_at, status, trigger, total_repos,
                  cloned_count, updated_count, failed_count, bytes_transferred, options_json
           FROM sync_runs ORDER BY started_at DESC LIMIT ?1"#,
    )?;
    let rows = stmt.query_map(params![limit as i64], row_to_run)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn get_one(conn: &Connection, id: i64) -> Result<Option<SyncRunRow>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT id, started_at, completed_at, status, trigger, total_repos,
                  cloned_count, updated_count, failed_count, bytes_transferred, options_json
           FROM sync_runs WHERE id = ?1"#,
    )?;
    let row = stmt.query_row(params![id], row_to_run).optional()?;
    Ok(row)
}

pub fn mark_orphaned_cancelled(conn: &Connection) -> Result<usize, SqliteErr> {
    let n = conn.execute(
        "UPDATE sync_runs SET status = 'cancelled', completed_at = datetime('now') WHERE status = 'running'",
        [],
    )?;
    Ok(n)
}

fn row_to_run(row: &Row<'_>) -> rusqlite::Result<SyncRunRow> {
    let started_at: String = row.get("started_at")?;
    let completed_at: Option<String> = row.get("completed_at")?;
    Ok(SyncRunRow {
        id: row.get("id")?,
        started_at: parse_dt(&started_at).unwrap_or_else(Utc::now),
        completed_at: completed_at.and_then(|s| parse_dt(&s)),
        status: row.get("status")?,
        trigger: row.get("trigger")?,
        total_repos: row.get("total_repos")?,
        cloned_count: row.get("cloned_count")?,
        updated_count: row.get("updated_count")?,
        failed_count: row.get("failed_count")?,
        bytes_transferred: row.get("bytes_transferred")?,
        options_json: row.get("options_json")?,
    })
}

fn parse_dt(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}
