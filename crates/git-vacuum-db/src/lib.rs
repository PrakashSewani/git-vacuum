pub mod connection;
pub mod migrations;
pub mod queries;

use std::path::Path;
use std::sync::Arc;

use git_vacuum_core::DbError;

/// Newtype wrapper around `rusqlite::Error`. We need this because `DbError`
/// lives in `git-vacuum-core` and `rusqlite::Error` lives in `rusqlite`, so
/// neither crate can implement `From<rusqlite::Error> for DbError` (orphan rule).
/// Internal query functions return `Result<T, SqliteErr>`; the public trait impl
/// converts via `?` to `DbError`.
#[derive(Debug)]
pub struct SqliteErr(pub rusqlite::Error);

impl From<SqliteErr> for DbError {
    fn from(e: SqliteErr) -> Self {
        DbError::Sqlite(e.0.to_string())
    }
}

impl From<rusqlite::Error> for SqliteErr {
    fn from(e: rusqlite::Error) -> Self {
        SqliteErr(e)
    }
}

use git_vacuum_core::{
    AttentionItem, DashboardStats, Database, LocalStatus, NewSyncEntry, NewSyncRun,
    RepoRow, SizeBucket, SyncRunUpdate, UserInfo,
};
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::queries::{accounts, repos, settings, stats, sync_entries, sync_runs};

pub struct SqliteDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteDatabase {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = connection::open_connection(path)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = connection::open_in_memory()?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> Result<R, SqliteErr>) -> Result<R, DbError> {
        let guard = self.conn.lock();
        f(&guard).map_err(Into::into)
    }
}

impl Database for SqliteDatabase {
    fn run_migrations(&self) -> Result<(), DbError> {
        self.with_conn(|c| migrations::run(c))
    }

    fn upsert_repos(&self, repos_list: &[RepoRow]) -> Result<(), DbError> {
        self.with_conn(|c| repos::upsert_repos(c, repos_list))
    }

    fn get_all_repos(&self) -> Result<Vec<RepoRow>, DbError> {
        self.with_conn(repos::get_all_repos)
    }

    fn get_repo(&self, github_id: i64) -> Result<Option<RepoRow>, DbError> {
        self.with_conn(|c| repos::get_repo(c, github_id))
    }

    fn update_local_status(&self, github_id: i64, status: &LocalStatus) -> Result<(), DbError> {
        self.with_conn(|c| repos::update_local_status(c, github_id, status))
    }

    fn set_repo_selected(&self, github_id: i64, selected: bool) -> Result<(), DbError> {
        self.with_conn(|c| repos::set_repo_selected(c, github_id, selected))
    }

    fn set_repos_selected(&self, github_ids: &[i64], selected: bool) -> Result<(), DbError> {
        self.with_conn(|c| repos::set_repos_selected(c, github_ids, selected))
    }

    fn mark_repo_deleted_on_remote(&self, github_id: i64) -> Result<(), DbError> {
        self.with_conn(|c| repos::mark_repo_deleted_on_remote(c, github_id))
    }

    fn prune_deleted_repos(&self) -> Result<usize, DbError> {
        self.with_conn(repos::prune_deleted_repos)
    }

    fn insert_sync_run(&self, run: &NewSyncRun) -> Result<i64, DbError> {
        self.with_conn(|c| sync_runs::insert(c, run))
    }

    fn update_sync_run(&self, id: i64, update: &SyncRunUpdate) -> Result<(), DbError> {
        self.with_conn(|c| sync_runs::update(c, id, update))
    }

    fn get_sync_runs(&self, limit: usize) -> Result<Vec<git_vacuum_core::SyncRunRow>, DbError> {
        self.with_conn(|c| sync_runs::get_many(c, limit))
    }

    fn get_sync_run(&self, id: i64) -> Result<Option<git_vacuum_core::SyncRunRow>, DbError> {
        self.with_conn(|c| sync_runs::get_one(c, id))
    }

    fn mark_orphaned_runs_cancelled(&self) -> Result<usize, DbError> {
        self.with_conn(sync_runs::mark_orphaned_cancelled)
    }

    fn insert_sync_entries(&self, entries: &[NewSyncEntry]) -> Result<(), DbError> {
        self.with_conn(|c| sync_entries::insert_many(c, entries))
    }

    fn get_sync_entries(&self, run_id: i64) -> Result<Vec<git_vacuum_core::SyncEntryRow>, DbError> {
        self.with_conn(|c| sync_entries::get_for_run(c, run_id))
    }

    fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        self.with_conn(|c| settings::get(c, key))
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.with_conn(|c| settings::set(c, key, value))
    }

    fn get_all_settings(&self) -> Result<Vec<(String, String)>, DbError> {
        self.with_conn(settings::get_all)
    }

    fn upsert_account(&self, info: &UserInfo) -> Result<(), DbError> {
        self.with_conn(|c| accounts::upsert(c, info))
    }

    fn get_active_account(&self) -> Result<Option<UserInfo>, DbError> {
        self.with_conn(accounts::get_active)
    }

    fn clear_active_account(&self) -> Result<(), DbError> {
        self.with_conn(accounts::clear_active)
    }

    fn get_dashboard_stats(&self) -> Result<DashboardStats, DbError> {
        self.with_conn(stats::dashboard)
    }

    fn get_attention_list(&self, limit: usize) -> Result<Vec<AttentionItem>, DbError> {
        self.with_conn(|c| stats::attention_list(c, limit))
    }

    fn get_size_distribution(&self) -> Result<Vec<SizeBucket>, DbError> {
        self.with_conn(stats::size_distribution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_vacuum_core::{CloneStatus, LocalStatus};

    fn make_db() -> SqliteDatabase {
        let db = SqliteDatabase::open_in_memory().unwrap();
        db.run_migrations().unwrap();
        db
    }

    fn make_repo(id: i64, name: &str) -> RepoRow {
        let now = chrono::Utc::now();
        RepoRow {
            id: 0,
            github_id: id,
            owner: "octocat".into(),
            name: name.into(),
            full_name: format!("octocat/{name}"),
            description: Some("test repo".into()),
            language: Some("Rust".into()),
            stars: 42,
            default_branch: "main".into(),
            visibility: "public".into(),
            is_fork: false,
            is_archived: false,
            clone_url_ssh: Some(format!("git@github.com:octocat/{name}.git")),
            clone_url_https: format!("https://github.com/octocat/{name}.git"),
            size_kb: Some(2048),
            pushed_at: Some(now),
            created_at: now,
            updated_at: now,
            clone_status: CloneStatus::NotCloned,
            local_path: None,
            local_size_kb: None,
            last_synced_at: None,
            last_error: None,
            behind_count: 0,
            selected: true,
            discovered_at: now,
            deleted_on_remote: false,
            topics_json: Some("[]".into()),
        }
    }

    #[test]
    fn migrations_and_settings() {
        let db = make_db();
        let path = db.get_setting("clone_path").unwrap();
        assert_eq!(path, Some("".to_string()));
        let conc = db.get_setting("default_concurrency").unwrap();
        assert_eq!(conc, Some("8".to_string()));
    }

    #[test]
    fn upsert_and_get_repos() {
        let db = make_db();
        let rows = vec![make_repo(101, "alpha"), make_repo(202, "beta")];
        db.upsert_repos(&rows).unwrap();
        let all = db.get_all_repos().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "alpha");
    }

    #[test]
    fn update_local_status() {
        let db = make_db();
        db.upsert_repos(&[make_repo(1, "x")]).unwrap();
        db.update_local_status(
            1,
            &LocalStatus {
                clone_status: CloneStatus::Cloned,
                local_path: Some("/tmp/x".into()),
                local_size_kb: Some(4096),
                last_synced_at: Some(chrono::Utc::now()),
                last_error: None,
                behind_count: 3,
            },
        )
        .unwrap();
        let r = db.get_repo(1).unwrap().unwrap();
        assert_eq!(r.behind_count, 3);
        assert_eq!(r.local_size_kb, Some(4096));
    }

    #[test]
    fn dashboard_and_attention() {
        let db = make_db();
        let mut r1 = make_repo(1, "a");
        r1.clone_status = CloneStatus::Cloned;
        r1.behind_count = 0;
        let mut r2 = make_repo(2, "b");
        r2.clone_status = CloneStatus::Cloned;
        r2.behind_count = 5;
        let mut r3 = make_repo(3, "c");
        r3.clone_status = CloneStatus::Error;
        db.upsert_repos(&[r1, r2, r3]).unwrap();
        let s = db.get_dashboard_stats().unwrap();
        assert_eq!(s.total_repos, 3);
        assert_eq!(s.behind, 1);
        assert_eq!(s.errors, 1);
        let att = db.get_attention_list(10).unwrap();
        assert_eq!(att.len(), 2);
    }
}
