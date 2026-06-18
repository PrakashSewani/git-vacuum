use chrono::{DateTime, Utc};
use git_vacuum_core::{AttentionItem, DashboardStats, SizeBucket};
use rusqlite::{params, Connection};

use crate::SqliteErr;

pub fn dashboard(conn: &Connection) -> Result<DashboardStats, SqliteErr> {
    let total_repos: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repos WHERE deleted_on_remote = 0",
        [],
        |r| r.get(0),
    )?;
    let up_to_date: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repos WHERE clone_status = 'cloned' AND behind_count = 0",
        [],
        |r| r.get(0),
    )?;
    let behind: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repos WHERE clone_status = 'cloned' AND behind_count > 0",
        [],
        |r| r.get(0),
    )?;
    let errors: i64 = conn.query_row(
        "SELECT COUNT(*) FROM repos WHERE clone_status = 'error'",
        [],
        |r| r.get(0),
    )?;
    let total_size: i64 = conn.query_row(
        "SELECT COALESCE(SUM(local_size_kb), 0) FROM repos",
        [],
        |r| r.get(0),
    )?;

    Ok(DashboardStats {
        total_repos: total_repos as usize,
        up_to_date: up_to_date as usize,
        behind: behind as usize,
        errors: errors as usize,
        total_size_bytes: (total_size as u64) * 1024,
    })
}

pub fn attention_list(conn: &Connection, limit: usize) -> Result<Vec<AttentionItem>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT full_name, clone_status, behind_count, last_error, last_synced_at, updated_at
           FROM repos
           WHERE deleted_on_remote = 0
             AND (clone_status IN ('error','stale')
                  OR (clone_status = 'cloned' AND behind_count > 0))
           ORDER BY
               CASE clone_status WHEN 'error' THEN 0 WHEN 'stale' THEN 1 ELSE 2 END,
               behind_count DESC,
               updated_at DESC
           LIMIT ?1"#,
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        let full_name: String = r.get(0)?;
        let status: String = r.get(1)?;
        let behind: i32 = r.get(2)?;
        let last_error: Option<String> = r.get(3)?;
        let last_synced: Option<String> = r.get(4)?;
        let updated: String = r.get(5)?;

        let reason = match status.as_str() {
            "error" => "error".to_string(),
            "stale" => "stale".to_string(),
            _ => format!("+{behind} commits behind"),
        };
        let detail = last_error.unwrap_or_else(|| {
            if behind > 0 {
                format!("{behind} new commits available")
            } else {
                String::new()
            }
        });
        let ts = last_synced
            .and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
            .or_else(|| {
                DateTime::parse_from_rfc3339(&updated)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
            .unwrap_or_else(Utc::now);
        Ok(AttentionItem {
            full_name,
            reason,
            detail,
            last_event_at: ts,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn size_distribution(conn: &Connection) -> Result<Vec<SizeBucket>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT
            CASE
                WHEN local_size_kb < 1024 THEN '<1MB'
                WHEN local_size_kb < 10240 THEN '1-10MB'
                WHEN local_size_kb < 102400 THEN '10-100MB'
                WHEN local_size_kb < 1048576 THEN '100MB-1GB'
                ELSE '>1GB'
            END AS bucket,
            COUNT(*) AS count
           FROM repos WHERE local_size_kb IS NOT NULL
           GROUP BY bucket
           ORDER BY MIN(local_size_kb)"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SizeBucket {
            label: r.get(0)?,
            count: r.get::<_, i64>(1)? as usize,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
