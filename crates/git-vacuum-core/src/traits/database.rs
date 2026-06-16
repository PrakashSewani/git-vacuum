use async_trait::async_trait;

use crate::types::activity::{SyncEntryRow, SyncRunRow};

use crate::types::repo::CloneStatus;


#[derive(Debug, Clone)]
pub struct RepoRow {
    pub github_id: i64,
    pub owner_login: String,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub default_branch: String,
    pub visibility: String,
    pub is_fork: bool,
    pub is_archived: bool,
    pub size_kb: Option<i64>,
    pub stars: i32,
    pub open_issues: i32,
    pub license_spdx: Option<String>,
    pub topics_json: Option<String>,
    pub clone_url_ssh: Option<String>,
    pub clone_url_https: Option<String>,
    pub pushed_at: Option<String>,
    pub clone_status: String,
    pub local_path: Option<String>,
    pub local_size_kb: Option<i64>,
    pub behind_count: i32,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub selected: bool,
    pub deleted_on_remote: bool,
    pub discovered_at: Option<String>,
}

#[async_trait]
pub trait Database: Send + Sync {
    async fn upsert_repos(&self, repos: &[RepoRow]) -> Result<(), String>;
    async fn get_all_repos(&self) -> Result<Vec<RepoRow>, String>;
    async fn update_clone_status(
        &self,
        full_name: &str,
        status: CloneStatus,
        local_path: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), String>;
    async fn mark_deleted_on_remote(&self, github_ids: &[i64]) -> Result<(), String>;
    async fn get_repos_to_sync(&self) -> Result<Vec<RepoRow>, String>;

    async fn insert_sync_run(&self, run: &SyncRunRow) -> Result<i64, String>;
    async fn update_sync_run(&self, run_id: i64, status: &str, completed_at: &str, duration_ms: i64) -> Result<(), String>;
    async fn get_sync_runs(&self, limit: usize) -> Result<Vec<SyncRunRow>, String>;
    async fn get_sync_entries(&self, run_id: i64) -> Result<Vec<SyncEntryRow>, String>;
    async fn insert_sync_entries(&self, entries: &[SyncEntryRow]) -> Result<(), String>;

    async fn get_setting(&self, key: &str) -> Result<Option<String>, String>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<(), String>;
    async fn get_all_settings(&self) -> Result<Vec<(String, String)>, String>;

    async fn upsert_account(&self, user_info: &crate::types::user::UserInfo) -> Result<(), String>;
    async fn upsert_orgs(&self, orgs: &[crate::types::org::OrgInfo]) -> Result<(), String>;
    async fn get_orgs(&self) -> Result<Vec<crate::types::org::OrgInfo>, String>;
}
