use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JobId(pub u64);

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "job-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlannedOperation {
    Clone,
    Sync,
    Mirror,
    Skip { reason: SkipReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkipReason {
    AlreadyUpToDate,
    LocalOnly,
    NoAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    High = 0,
    Normal = 1,
    Low = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    pub job_id: JobId,
    pub repo_full_name: String,
    pub repo_github_id: i64,
    pub owner_login: String,
    pub clone_url: String,
    pub local_path: PathBuf,
    pub operation: PlannedOperation,
    pub priority: Priority,
    pub attempt: u32,
}
