use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunRow {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: SyncRunStatus,
    pub trigger: SyncTrigger,
    pub total_repos: i32,
    pub cloned_count: i32,
    pub updated_count: i32,
    pub skipped_count: i32,
    pub failed_count: i32,
    pub bytes_transferred: i64,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncRunStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl std::fmt::Display for SyncRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncRunStatus::Running => write!(f, "running"),
            SyncRunStatus::Completed => write!(f, "completed"),
            SyncRunStatus::Cancelled => write!(f, "cancelled"),
            SyncRunStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncTrigger {
    Manual,
    Scheduled,
    Cli,
}

impl std::fmt::Display for SyncTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncTrigger::Manual => write!(f, "manual"),
            SyncTrigger::Scheduled => write!(f, "scheduled"),
            SyncTrigger::Cli => write!(f, "cli"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntryRow {
    pub id: i64,
    pub run_id: i64,
    pub repo_id: i64,
    pub repo_full_name: String,
    pub owner_login: String,
    pub operation: String,
    pub entry_status: String,
    pub bytes_transferred: i64,
    pub new_commits: i32,
    pub duration_ms: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
