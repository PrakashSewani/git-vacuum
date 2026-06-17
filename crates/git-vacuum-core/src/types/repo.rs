use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoVisibility {
    Public,
    Private,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloneStatus {
    NotCloned,
    Cloned,
    Stale,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RepoSource {
    MyRepos,
    Org { login: String },
    Starred,
    All,
}

impl RepoSource {
    pub fn label(&self) -> String {
        match self {
            RepoSource::MyRepos => "My Repos".into(),
            RepoSource::Org { login } => format!("Org: {login}"),
            RepoSource::Starred => "Starred".into(),
            RepoSource::All => "All Accessible".into(),
        }
    }
}

/// Repository data as it comes from the GitHub API.
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_is_org: bool,
}

/// Repository as the UI sees it: merged remote data + local cache + filesystem state.
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
    pub size_kb: Option<i64>,
    pub stars: i32,
    pub pushed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub topics: Vec<String>,
    pub clone_url_https: String,
    pub clone_url_ssh: Option<String>,

    pub clone_status: CloneStatus,
    pub local_path: Option<String>,
    pub local_size_kb: Option<i64>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub behind_count: i32,

    pub selected: bool,
    pub deleted_on_remote: bool,
    pub discovered_at: DateTime<Utc>,
}
