use chrono::{DateTime, Utc};
use git_vacuum_core::{CloneStatus, LocalStatus, RepoRow};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::SqliteErr;

pub fn upsert_repos(conn: &Connection, repos: &[RepoRow]) -> Result<(), SqliteErr> {
    let tx = conn.unchecked_transaction()?;
    for r in repos {
        tx.execute(
            r#"
            INSERT INTO repos (
                github_id, owner, name, full_name, description, language, stars,
                default_branch, visibility, is_fork, is_archived, clone_url_ssh,
                clone_url_https, size_kb, pushed_at, created_at, updated_at,
                topics_json, discovered_at,
                clone_status, local_path, local_size_kb, last_synced_at, last_error,
                behind_count, selected
            ) VALUES (
                ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                ?20,?21,?22,?23,?24,?25,?26
            )
            ON CONFLICT(github_id) DO UPDATE SET
                owner           = excluded.owner,
                name            = excluded.name,
                full_name       = excluded.full_name,
                description     = excluded.description,
                language        = excluded.language,
                stars           = excluded.stars,
                default_branch  = excluded.default_branch,
                visibility      = excluded.visibility,
                is_fork         = excluded.is_fork,
                is_archived     = excluded.is_archived,
                clone_url_ssh   = excluded.clone_url_ssh,
                clone_url_https = excluded.clone_url_https,
                size_kb         = excluded.size_kb,
                pushed_at       = excluded.pushed_at,
                updated_at      = excluded.updated_at,
                topics_json     = excluded.topics_json,
                deleted_on_remote = 0
            "#,
            params![
                r.github_id,
                r.owner,
                r.name,
                r.full_name,
                r.description,
                r.language,
                r.stars,
                r.default_branch,
                r.visibility,
                r.is_fork as i32,
                r.is_archived as i32,
                r.clone_url_ssh,
                r.clone_url_https,
                r.size_kb,
                r.pushed_at.map(|d| d.to_rfc3339()),
                r.created_at.to_rfc3339(),
                r.updated_at.to_rfc3339(),
                r.topics_json,
                r.discovered_at.to_rfc3339(),
                clone_status_to_str(r.clone_status),
                r.local_path,
                r.local_size_kb,
                r.last_synced_at.map(|d| d.to_rfc3339()),
                r.last_error,
                r.behind_count,
                r.selected as i32,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_all_repos(conn: &Connection) -> Result<Vec<RepoRow>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT id, github_id, owner, name, full_name, description, language, stars,
                  default_branch, visibility, is_fork, is_archived, clone_url_ssh,
                  clone_url_https, size_kb, pushed_at, created_at, updated_at,
                  clone_status, local_path, local_size_kb, last_synced_at, last_error,
                  behind_count, selected, discovered_at, deleted_on_remote, topics_json
           FROM repos ORDER BY owner, name"#,
    )?;
    let rows = stmt.query_map([], row_to_repo)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn get_repo(conn: &Connection, github_id: i64) -> Result<Option<RepoRow>, SqliteErr> {
    let mut stmt = conn.prepare(
        r#"SELECT id, github_id, owner, name, full_name, description, language, stars,
                  default_branch, visibility, is_fork, is_archived, clone_url_ssh,
                  clone_url_https, size_kb, pushed_at, created_at, updated_at,
                  clone_status, local_path, local_size_kb, last_synced_at, last_error,
                  behind_count, selected, discovered_at, deleted_on_remote, topics_json
           FROM repos WHERE github_id = ?1"#,
    )?;
    let row = stmt.query_row(params![github_id], row_to_repo).optional()?;
    Ok(row)
}

fn row_to_repo(row: &Row<'_>) -> rusqlite::Result<RepoRow> {
    let visibility: String = row.get("visibility")?;
    let clone_status: String = row.get("clone_status")?;
    let is_fork: i32 = row.get("is_fork")?;
    let is_archived: i32 = row.get("is_archived")?;
    let selected: i32 = row.get("selected")?;
    let deleted_on_remote: i32 = row.get("deleted_on_remote")?;
    let created_at: String = row.get("created_at")?;
    let updated_at: String = row.get("updated_at")?;
    let discovered_at: String = row.get("discovered_at")?;
    let pushed_at: Option<String> = row.get("pushed_at")?;
    let last_synced_at: Option<String> = row.get("last_synced_at")?;

    Ok(RepoRow {
        id: row.get("id")?,
        github_id: row.get("github_id")?,
        owner: row.get("owner")?,
        name: row.get("name")?,
        full_name: row.get("full_name")?,
        description: row.get("description")?,
        language: row.get("language")?,
        stars: row.get("stars")?,
        default_branch: row.get("default_branch")?,
        visibility,
        is_fork: is_fork != 0,
        is_archived: is_archived != 0,
        clone_url_ssh: row.get("clone_url_ssh")?,
        clone_url_https: row.get("clone_url_https")?,
        size_kb: row.get("size_kb")?,
        pushed_at: pushed_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        }),
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        clone_status: parse_clone_status(&clone_status),
        local_path: row.get("local_path")?,
        local_size_kb: row.get("local_size_kb")?,
        last_synced_at: last_synced_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        }),
        last_error: row.get("last_error")?,
        behind_count: row.get("behind_count")?,
        selected: selected != 0,
        discovered_at: DateTime::parse_from_rfc3339(&discovered_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        deleted_on_remote: deleted_on_remote != 0,
        topics_json: row.get("topics_json")?,
    })
}

fn parse_clone_status(s: &str) -> CloneStatus {
    match s {
        "cloned" => CloneStatus::Cloned,
        "stale" => CloneStatus::Stale,
        "error" => CloneStatus::Error,
        _ => CloneStatus::NotCloned,
    }
}

pub fn update_local_status(
    conn: &Connection,
    github_id: i64,
    status: &LocalStatus,
) -> Result<(), SqliteErr> {
    conn.execute(
        r#"UPDATE repos SET
            clone_status   = ?1,
            local_path     = ?2,
            local_size_kb  = ?3,
            last_synced_at = ?4,
            last_error     = ?5,
            behind_count   = ?6
           WHERE github_id = ?7"#,
        params![
            clone_status_to_str(status.clone_status),
            status.local_path,
            status.local_size_kb,
            status.last_synced_at.map(|d| d.to_rfc3339()),
            status.last_error,
            status.behind_count,
            github_id,
        ],
    )?;
    Ok(())
}

pub fn set_repo_selected(
    conn: &Connection,
    github_id: i64,
    selected: bool,
) -> Result<(), SqliteErr> {
    conn.execute(
        "UPDATE repos SET selected = ?1 WHERE github_id = ?2",
        params![selected as i32, github_id],
    )?;
    Ok(())
}

pub fn set_repos_selected(
    conn: &Connection,
    github_ids: &[i64],
    selected: bool,
) -> Result<(), SqliteErr> {
    if github_ids.is_empty() {
        return Ok(());
    }
    let placeholders: Vec<String> = (0..github_ids.len())
        .map(|i| format!("?{}", i + 2))
        .collect();
    let sql = format!(
        "UPDATE repos SET selected = ?1 WHERE github_id IN ({})",
        placeholders.join(",")
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(github_ids.len() + 1);
    params_vec.push(Box::new(selected));
    for id in github_ids {
        params_vec.push(Box::new(*id));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = params_vec
        .iter()
        .map(|p| p.as_ref() as &dyn rusqlite::ToSql)
        .collect();
    conn.execute(&sql, refs.as_slice())?;
    Ok(())
}

pub fn mark_repo_deleted_on_remote(conn: &Connection, github_id: i64) -> Result<(), SqliteErr> {
    conn.execute(
        "UPDATE repos SET deleted_on_remote = 1 WHERE github_id = ?1",
        params![github_id],
    )?;
    Ok(())
}

pub fn prune_deleted_repos(conn: &Connection) -> Result<usize, SqliteErr> {
    let n = conn.execute("DELETE FROM repos WHERE deleted_on_remote = 1", [])?;
    Ok(n)
}

fn clone_status_to_str(s: CloneStatus) -> &'static str {
    match s {
        CloneStatus::NotCloned => "not_cloned",
        CloneStatus::Cloned => "cloned",
        CloneStatus::Stale => "stale",
        CloneStatus::Error => "error",
    }
}
