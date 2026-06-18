use chrono::{DateTime, Utc};
use git_vacuum_core::UserInfo;
use rusqlite::{params, Connection, OptionalExtension};

use crate::SqliteErr;

pub fn upsert(conn: &Connection, info: &UserInfo) -> Result<(), SqliteErr> {
    let scopes_json = serde_json::to_string(&info.scopes).unwrap_or_else(|_| "[]".to_string());
    let token_expires = info.token_expires_at.map(|d| d.to_rfc3339());
    conn.execute(
        r#"INSERT INTO accounts
            (github_user_id, login, name, email, avatar_url, scopes_json, token_expires_at, last_validated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
           ON CONFLICT(github_user_id) DO UPDATE SET
                login = excluded.login,
                name = excluded.name,
                email = excluded.email,
                avatar_url = excluded.avatar_url,
                scopes_json = excluded.scopes_json,
                token_expires_at = excluded.token_expires_at,
                last_validated_at = datetime('now')"#,
        params![
            info.github_user_id,
            info.login,
            info.name,
            info.email,
            info.avatar_url,
            scopes_json,
            token_expires,
        ],
    )?;
    Ok(())
}

pub fn get_active(conn: &Connection) -> Result<Option<UserInfo>, SqliteErr> {
    #[allow(clippy::type_complexity)]
    let row: Option<(
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
    )> = conn
        .query_row(
            r#"SELECT github_user_id, login, name, email, avatar_url, scopes_json, token_expires_at
               FROM accounts ORDER BY last_validated_at DESC LIMIT 1"#,
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .optional()?;

    if let Some((id, login, name, email, avatar, scopes_json, expires)) = row {
        let scopes: Vec<String> = serde_json::from_str(&scopes_json).unwrap_or_default();
        let token_expires_at = expires.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        });
        Ok(Some(UserInfo {
            github_user_id: id,
            login,
            name,
            email,
            avatar_url: avatar,
            scopes,
            token_expires_at,
        }))
    } else {
        Ok(None)
    }
}

pub fn clear_active(conn: &Connection) -> Result<(), SqliteErr> {
    conn.execute("DELETE FROM accounts", [])?;
    Ok(())
}
