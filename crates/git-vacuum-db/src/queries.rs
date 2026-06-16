use chrono::Utc;
use git_vacuum_core::traits::database::RepoRow;
use git_vacuum_core::traits::Database;
use git_vacuum_core::types::activity::{SyncEntryRow, SyncRunRow};
use git_vacuum_core::types::org::OrgInfo;
use git_vacuum_core::types::repo::CloneStatus;
use git_vacuum_core::types::user::UserInfo;
use crate::connection::ConnectionPool;

macro_rules! param {
    ($v:expr) => {
        Box::new($v) as Box<dyn rusqlite::types::ToSql + Send + Sync>
    };
}

pub struct SqliteDatabase {
    pool: ConnectionPool,
}

impl SqliteDatabase {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    fn now_str() -> String {
        Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
}

#[async_trait::async_trait]
impl Database for SqliteDatabase {
    async fn upsert_repos(&self, repos: &[RepoRow]) -> Result<(), String> {
        for repo in repos {
            let params: Vec<Box<dyn rusqlite::types::ToSql + Send + Sync>> = vec![
                param!(repo.github_id), param!(repo.owner_login.clone()),
                param!(repo.name.clone()), param!(repo.full_name.clone()),
                param!(repo.description.clone()), param!(repo.language.clone()),
                param!(repo.default_branch.clone()), param!(repo.visibility.clone()),
                param!(repo.is_fork as i32), param!(repo.is_archived as i32),
                param!(false as i32), param!(repo.size_kb),
                param!(repo.stars), param!(repo.open_issues),
                param!(repo.license_spdx.clone()), param!(repo.topics_json.clone()),
                param!(repo.clone_url_ssh.clone()), param!(repo.clone_url_https.clone()),
                param!(repo.pushed_at.clone()), param!(repo.deleted_on_remote as i32),
                param!(repo.discovered_at.clone()), param!(Self::now_str()),
            ];
            self.pool.execute(
                "INSERT INTO repositories (
                    github_id, owner_login, name, full_name, description, language,
                    default_branch, visibility, is_fork, is_archived, is_template,
                    size_kb, stars, open_issues, license_spdx, topics_json,
                    clone_url_ssh, clone_url_https, pushed_at,
                    deleted_on_remote, discovered_at, updated_at
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
                ON CONFLICT(github_id) DO UPDATE SET
                    owner_login=?2, name=?3, full_name=?4, description=?5, language=?6,
                    default_branch=?7, visibility=?8, is_fork=?9, is_archived=?10, is_template=?11,
                    size_kb=?12, stars=?13, open_issues=?14, license_spdx=?15, topics_json=?16,
                    clone_url_ssh=?17, clone_url_https=?18, pushed_at=?19,
                    deleted_on_remote=?20, discovered_at=?21, updated_at=?22",
                &params,
            ).await?;
        }
        Ok(())
    }

    async fn get_all_repos(&self) -> Result<Vec<RepoRow>, String> {
        self.pool.query_map(
            "SELECT github_id, owner_login, name, full_name, description, language,
                    default_branch, visibility, is_fork, is_archived, size_kb, stars,
                    open_issues, license_spdx, topics_json, clone_url_ssh, clone_url_https,
                    pushed_at, 'not_cloned', NULL, NULL, 0, NULL, NULL, 1, deleted_on_remote, discovered_at
             FROM repositories WHERE deleted_on_remote = 0 ORDER BY owner_login, name",
            &[],
            |row| Ok(RepoRow {
                github_id: row.get(0)?,
                owner_login: row.get(1)?,
                name: row.get(2)?,
                full_name: row.get(3)?,
                description: row.get(4)?,
                language: row.get(5)?,
                default_branch: row.get(6)?,
                visibility: row.get(7)?,
                is_fork: row.get::<_, i32>(8)? != 0,
                is_archived: row.get::<_, i32>(9)? != 0,
                size_kb: row.get(10)?,
                stars: row.get(11)?,
                open_issues: row.get(12)?,
                license_spdx: row.get(13)?,
                topics_json: row.get(14)?,
                clone_url_ssh: row.get(15)?,
                clone_url_https: row.get(16)?,
                pushed_at: row.get(17)?,
                clone_status: row.get(18)?,
                local_path: row.get(19)?,
                local_size_kb: row.get(20)?,
                behind_count: row.get(21)?,
                last_synced_at: row.get(22)?,
                last_error: row.get(23)?,
                selected: row.get::<_, i32>(24)? != 0,
                deleted_on_remote: row.get::<_, i32>(25)? != 0,
                discovered_at: row.get(26)?,
            }),
        ).await
    }

    async fn update_clone_status(
        &self,
        full_name: &str,
        status: CloneStatus,
        local_path: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), String> {
        let repo_id: i64 = self.pool.query_row(
            "SELECT id FROM repositories WHERE full_name = ?1",
            &[param!(full_name.to_string())],
            |row| row.get(0),
        ).await?;
        let status_str = status.to_string();
        let params: Vec<Box<dyn rusqlite::types::ToSql + Send + Sync>> = vec![
            param!(repo_id), param!(local_path.map(|s| s.to_string())),
            param!(status_str.clone()), param!(error.map(|s| s.to_string())), param!(Self::now_str()),
        ];
        self.pool.execute(
            "INSERT INTO local_clones (repo_id, local_path, clone_status, last_error, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(repo_id) DO UPDATE SET
                clone_status = ?3, last_error = ?4, updated_at = ?5",
            &params,
        ).await?;
        Ok(())
    }

    async fn mark_deleted_on_remote(&self, github_ids: &[i64]) -> Result<(), String> {
        if github_ids.is_empty() { return Ok(()); }
        let placeholders: Vec<String> = github_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!("UPDATE repositories SET deleted_on_remote = 1, updated_at = ?{} WHERE github_id IN ({})",
            github_ids.len() + 1, placeholders.join(","));
        let mut params: Vec<Box<dyn rusqlite::types::ToSql + Send + Sync>> = github_ids.iter().map(|i| param!(*i)).collect();
        params.push(param!(Self::now_str()));
        self.pool.execute(&sql, &params).await?;
        Ok(())
    }

    async fn get_repos_to_sync(&self) -> Result<Vec<RepoRow>, String> {
        self.get_all_repos().await
    }

    async fn insert_sync_run(&self, run: &SyncRunRow) -> Result<i64, String> {
        let started_at = run.started_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let status = run.status.to_string();
        let trigger = run.trigger.to_string();
        self.pool.execute(
            "INSERT INTO sync_runs (started_at, status, trigger, total_repos) VALUES (?1, ?2, ?3, ?4)",
            &[param!(started_at), param!(status), param!(trigger), param!(run.total_repos)],
        ).await?;
        self.pool.last_insert_rowid().await
    }

    async fn update_sync_run(&self, run_id: i64, status: &str, completed_at: &str, duration_ms: i64) -> Result<(), String> {
        self.pool.execute(
            "UPDATE sync_runs SET status = ?1, completed_at = ?2, duration_ms = ?3 WHERE id = ?4",
            &[param!(status.to_string()), param!(completed_at.to_string()), param!(duration_ms), param!(run_id)],
        ).await?;
        Ok(())
    }

    async fn get_sync_runs(&self, limit: usize) -> Result<Vec<SyncRunRow>, String> {
        self.pool.query_map(
            "SELECT id, started_at, completed_at, status, trigger, total_repos,
                    cloned_count, updated_count, skipped_count, failed_count,
                    bytes_transferred, duration_ms
             FROM sync_runs ORDER BY started_at DESC LIMIT ?1",
            &[param!(limit as i64)],
            |row| {
                let status_str: String = row.get(3)?;
                let trigger_str: String = row.get(4)?;
                let started: String = row.get(1)?;
                let completed: Option<String> = row.get(2)?;
                Ok(SyncRunRow {
                    id: row.get(0)?,
                    started_at: chrono::DateTime::parse_from_rfc3339(&started).map(|dt| dt.with_timezone(&chrono::Utc)).unwrap_or_default(),
                    completed_at: completed.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&chrono::Utc))),
                    status: match status_str.as_str() {
                        "completed" => git_vacuum_core::SyncRunStatus::Completed,
                        "cancelled" => git_vacuum_core::SyncRunStatus::Cancelled,
                        "failed" => git_vacuum_core::SyncRunStatus::Failed,
                        _ => git_vacuum_core::SyncRunStatus::Running,
                    },
                    trigger: match trigger_str.as_str() {
                        "scheduled" => git_vacuum_core::SyncTrigger::Scheduled,
                        "cli" => git_vacuum_core::SyncTrigger::Cli,
                        _ => git_vacuum_core::SyncTrigger::Manual,
                    },
                    total_repos: row.get(5)?,
                    cloned_count: row.get(6)?,
                    updated_count: row.get(7)?,
                    skipped_count: row.get(8)?,
                    failed_count: row.get(9)?,
                    bytes_transferred: row.get(10)?,
                    duration_ms: row.get(11)?,
                })
            },
        ).await
    }

    async fn get_sync_entries(&self, run_id: i64) -> Result<Vec<SyncEntryRow>, String> {
        self.pool.query_map(
            "SELECT se.id, se.run_id, se.repo_id, r.full_name, r.owner_login,
                    se.operation, se.entry_status, se.bytes_transferred,
                    se.new_commits, se.duration_ms, se.error_code, se.error_message,
                    se.started_at, se.completed_at
             FROM sync_entries se JOIN repositories r ON r.id = se.repo_id
             WHERE se.run_id = ?1 ORDER BY se.started_at",
            &[param!(run_id)],
            |row| {
                let started: Option<String> = row.get(12)?;
                let completed: Option<String> = row.get(13)?;
                Ok(SyncEntryRow {
                    id: row.get(0)?, run_id: row.get(1)?, repo_id: row.get(2)?,
                    repo_full_name: row.get(3)?, owner_login: row.get(4)?,
                    operation: row.get(5)?, entry_status: row.get(6)?,
                    bytes_transferred: row.get(7)?, new_commits: row.get(8)?,
                    duration_ms: row.get(9)?, error_code: row.get(10)?,
                    error_message: row.get(11)?,
                    started_at: started.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&chrono::Utc))),
                    completed_at: completed.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&chrono::Utc))),
                })
            },
        ).await
    }

    async fn insert_sync_entries(&self, entries: &[SyncEntryRow]) -> Result<(), String> {
        for entry in entries {
            let started = entry.started_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string());
            let completed = entry.completed_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string());
            self.pool.execute(
                "INSERT INTO sync_entries (run_id, repo_id, operation, entry_status,
                    bytes_transferred, new_commits, duration_ms, error_code, error_message,
                    started_at, completed_at)
                 SELECT ?1, r.id, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11
                 FROM repositories r WHERE r.full_name = ?2",
                &[param!(entry.run_id), param!(entry.repo_full_name.clone()), param!(entry.operation.clone()),
                  param!(entry.entry_status.clone()), param!(entry.bytes_transferred), param!(entry.new_commits),
                  param!(entry.duration_ms), param!(entry.error_code.clone()), param!(entry.error_message.clone()),
                  param!(started), param!(completed)],
            ).await?;
        }
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        match self.pool.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            &[param!(key.to_string())],
            |row| row.get(0),
        ).await {
            Ok(v) => Ok(Some(v)),
            Err(_) => Ok(None),
        }
    }

    async fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.pool.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            &[param!(key.to_string()), param!(value.to_string()), param!(Self::now_str())],
        ).await?;
        Ok(())
    }

    async fn get_all_settings(&self) -> Result<Vec<(String, String)>, String> {
        self.pool.query_map(
            "SELECT key, value FROM settings",
            &[],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).await
    }

    async fn upsert_account(&self, user_info: &UserInfo) -> Result<(), String> {
        let scopes = user_info.scopes.join(",");
        let expires = user_info.token_expires_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string());
        self.pool.execute(
            "INSERT INTO github_accounts (github_user_id, login, display_name, email, avatar_url,
                token_scopes, token_expires_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(github_user_id) DO UPDATE SET
                login=?2, display_name=?3, email=?4, avatar_url=?5,
                token_scopes=?6, token_expires_at=?7, updated_at=?8",
            &[param!(user_info.github_user_id), param!(user_info.login.clone()),
              param!(user_info.display_name.clone()), param!(user_info.email.clone()),
              param!(user_info.avatar_url.clone()), param!(scopes), param!(expires), param!(Self::now_str())],
        ).await?;
        Ok(())
    }

    async fn upsert_orgs(&self, orgs: &[OrgInfo]) -> Result<(), String> {
        for org in orgs {
            self.pool.execute(
                "INSERT INTO github_orgs (github_org_id, login, display_name, description, avatar_url,
                    repos_count, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(github_org_id) DO UPDATE SET
                    login=?2, display_name=?3, description=?4, avatar_url=?5,
                    repos_count=?6, updated_at=?7",
                &[param!(org.github_org_id), param!(org.login.clone()), param!(org.display_name.clone()),
                  param!(org.description.clone()), param!(org.avatar_url.clone()),
                  param!(org.repos_count), param!(Self::now_str())],
            ).await?;
        }
        Ok(())
    }

    async fn get_orgs(&self) -> Result<Vec<OrgInfo>, String> {
        self.pool.query_map(
            "SELECT github_org_id, login, display_name, description, avatar_url, repos_count
             FROM github_orgs ORDER BY login",
            &[],
            |row| Ok(OrgInfo {
                github_org_id: row.get(0)?,
                login: row.get(1)?,
                display_name: row.get(2)?,
                description: row.get(3)?,
                avatar_url: row.get(4)?,
                repos_count: row.get(5)?,
                discovered_at: chrono::Utc::now(),
            }),
        ).await
    }
}
