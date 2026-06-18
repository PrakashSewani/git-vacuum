use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::types::activity::{SyncEntryRow, SyncRunRow};
use crate::types::repo::CloneStatus;
use crate::types::user::UserInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRow {
    pub id: i64,
    pub github_id: i64,
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars: i32,
    pub default_branch: String,
    pub visibility: String,
    pub is_fork: bool,
    pub is_archived: bool,
    pub clone_url_ssh: Option<String>,
    pub clone_url_https: String,
    pub size_kb: Option<i64>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub clone_status: CloneStatus,
    pub local_path: Option<String>,
    pub local_size_kb: Option<i64>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub behind_count: i32,
    pub selected: bool,
    pub discovered_at: DateTime<Utc>,
    pub deleted_on_remote: bool,
    pub topics_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalStatus {
    pub clone_status: CloneStatus,
    pub local_path: Option<String>,
    pub local_size_kb: Option<i64>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub behind_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_repos: usize,
    pub up_to_date: usize,
    pub behind: usize,
    pub errors: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionItem {
    pub full_name: String,
    pub reason: String,
    pub detail: String,
    pub last_event_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeBucket {
    pub label: String,
    pub count: usize,
}

pub trait Database: Send + Sync {
    fn run_migrations(&self) -> Result<(), crate::error::DbError>;

    fn upsert_repos(&self, repos: &[RepoRow]) -> Result<(), crate::error::DbError>;
    fn get_all_repos(&self) -> Result<Vec<RepoRow>, crate::error::DbError>;
    fn get_repo(&self, github_id: i64) -> Result<Option<RepoRow>, crate::error::DbError>;
    fn update_local_status(
        &self,
        github_id: i64,
        status: &LocalStatus,
    ) -> Result<(), crate::error::DbError>;
    fn set_repo_selected(
        &self,
        github_id: i64,
        selected: bool,
    ) -> Result<(), crate::error::DbError>;
    fn set_repos_selected(
        &self,
        github_ids: &[i64],
        selected: bool,
    ) -> Result<(), crate::error::DbError>;
    fn mark_repo_deleted_on_remote(&self, github_id: i64) -> Result<(), crate::error::DbError>;
    fn prune_deleted_repos(&self) -> Result<usize, crate::error::DbError>;

    fn insert_sync_run(&self, run: &NewSyncRun) -> Result<i64, crate::error::DbError>;
    fn update_sync_run(&self, id: i64, update: &SyncRunUpdate)
        -> Result<(), crate::error::DbError>;
    fn get_sync_runs(&self, limit: usize) -> Result<Vec<SyncRunRow>, crate::error::DbError>;
    fn get_sync_run(&self, id: i64) -> Result<Option<SyncRunRow>, crate::error::DbError>;
    fn mark_orphaned_runs_cancelled(&self) -> Result<usize, crate::error::DbError>;
    fn insert_sync_entries(&self, entries: &[NewSyncEntry]) -> Result<(), crate::error::DbError>;
    fn get_sync_entries(&self, run_id: i64) -> Result<Vec<SyncEntryRow>, crate::error::DbError>;

    fn get_setting(&self, key: &str) -> Result<Option<String>, crate::error::DbError>;
    fn set_setting(&self, key: &str, value: &str) -> Result<(), crate::error::DbError>;
    fn get_all_settings(&self) -> Result<Vec<(String, String)>, crate::error::DbError>;

    fn upsert_account(&self, info: &UserInfo) -> Result<(), crate::error::DbError>;
    fn get_active_account(&self) -> Result<Option<UserInfo>, crate::error::DbError>;
    fn clear_active_account(&self) -> Result<(), crate::error::DbError>;

    fn get_dashboard_stats(&self) -> Result<DashboardStats, crate::error::DbError>;
    fn get_attention_list(&self, limit: usize)
        -> Result<Vec<AttentionItem>, crate::error::DbError>;
    fn get_size_distribution(&self) -> Result<Vec<SizeBucket>, crate::error::DbError>;
}

#[derive(Debug, Clone)]
pub struct NewSyncRun {
    pub started_at: DateTime<Utc>,
    pub trigger: String,
    pub total_repos: i32,
    pub options_json: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SyncRunUpdate {
    pub completed_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub cloned_count: Option<i32>,
    pub updated_count: Option<i32>,
    pub failed_count: Option<i32>,
    pub bytes_transferred: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewSyncEntry {
    pub run_id: i64,
    pub repo_id: i64,
    pub operation: String,
    pub status: String,
    pub bytes_transferred: i64,
    pub new_commits: i32,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
}

pub trait DatabaseFactory: Send + Sync {
    fn open(&self, path: &Path) -> Result<Box<dyn Database>, crate::error::DbError>;
}
