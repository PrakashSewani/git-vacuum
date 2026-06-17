use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Tsv,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunRow {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub trigger: String,
    pub total_repos: i32,
    pub cloned_count: i32,
    pub updated_count: i32,
    pub failed_count: i32,
    pub bytes_transferred: i64,
    pub options_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntryRow {
    pub id: i64,
    pub run_id: i64,
    pub repo_id: i64,
    pub operation: String,
    pub status: String,
    pub bytes_transferred: i64,
    pub new_commits: i32,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
}
