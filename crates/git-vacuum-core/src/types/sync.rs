
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub concurrency: usize,
    pub protocol: CloneProtocol,
    pub timeout_per_job: Duration,
    pub mirror: bool,
    pub include_wikis: bool,
    pub fetch_lfs: bool,
    pub prune_deleted: bool,
    pub retry_failed: bool,
    pub max_retries: u32,
    pub retry_delay_base: Duration,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            concurrency: 8,
            protocol: CloneProtocol::Ssh,
            timeout_per_job: Duration::from_secs(1800),
            mirror: false,
            include_wikis: false,
            fetch_lfs: false,
            prune_deleted: false,
            retry_failed: true,
            max_retries: 2,
            retry_delay_base: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CloneProtocol {
    Ssh,
    Https { token: String },
}

#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub run_id: i64,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub total_repos: usize,
    pub cloned_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub bytes_transferred: u64,
    pub duration: Duration,
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone)]
pub struct JobSummary {
    pub repo_full_name: String,
    pub outcome: JobOutcomeSummary,
    pub duration: Duration,
    pub attempts: u32,
}

#[derive(Debug, Clone)]
pub enum JobOutcomeSummary {
    Cloned { bytes: u64 },
    Synced { new_commits: u32, bytes: u64 },
    UpToDate,
    Failed { error: String },
    Skipped { reason: String },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct PartialSyncSummary {
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub pending_dropped: usize,
    pub bytes_transferred: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryScope {
    MyRepos,
    Org(String),
    Starred,
    All,
}
