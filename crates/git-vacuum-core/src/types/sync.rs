use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloneProtocol {
    Ssh,
    Https,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOptions {
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
            timeout_per_job: Duration::from_secs(30 * 60),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummary {
    pub total_jobs: usize,
    pub cloned: usize,
    pub updated: usize,
    pub up_to_date: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: usize,
    pub bytes_transferred: u64,
    pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSyncSummary {
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub pending_dropped: usize,
    pub bytes_transferred: u64,
}
