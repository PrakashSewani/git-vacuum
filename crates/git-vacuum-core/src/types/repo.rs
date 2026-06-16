use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRepo {
    pub github_id: i64,
    pub owner_login: String,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub default_branch: String,
    pub visibility: RepoVisibility,
    pub is_fork: bool,
    pub is_archived: bool,
    pub is_template: bool,
    pub size_kb: Option<i64>,
    pub stars: i32,
    pub open_issues: i32,
    pub license_spdx: Option<String>,
    pub topics: Vec<String>,
    pub clone_url_ssh: Option<String>,
    pub clone_url_https: String,
    pub homepage_url: Option<String>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub owner_is_org: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RepoVisibility {
    Public,
    Private,
    Internal,
}

impl std::fmt::Display for RepoVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoVisibility::Public => write!(f, "public"),
            RepoVisibility::Private => write!(f, "private"),
            RepoVisibility::Internal => write!(f, "internal"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub github_id: i64,
    pub owner_login: String,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub default_branch: String,
    pub visibility: RepoVisibility,
    pub is_fork: bool,
    pub is_archived: bool,
    pub is_template: bool,
    pub size_kb: Option<i64>,
    pub stars: i32,
    pub open_issues: i32,
    pub license_spdx: Option<String>,
    pub topics: Vec<String>,
    pub clone_url_ssh: Option<String>,
    pub clone_url_https: String,
    pub homepage_url: Option<String>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub clone_status: CloneStatus,
    pub local_path: Option<String>,
    pub local_size_kb: Option<i64>,
    pub behind_count: i32,
    pub ahead_count: i32,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub selected: bool,
    pub deleted_on_remote: bool,
    pub discovered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CloneStatus {
    NotCloned,
    Cloning,
    Cloned,
    Stale,
    Error,
}

impl std::fmt::Display for CloneStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloneStatus::NotCloned => write!(f, "not_cloned"),
            CloneStatus::Cloning => write!(f, "cloning"),
            CloneStatus::Cloned => write!(f, "cloned"),
            CloneStatus::Stale => write!(f, "stale"),
            CloneStatus::Error => write!(f, "error"),
        }
    }
}
