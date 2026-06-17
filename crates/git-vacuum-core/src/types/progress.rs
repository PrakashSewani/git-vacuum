use serde::{Deserialize, Serialize};

use super::job::JobId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobPhase {
    Queued,
    Connecting,
    Receiving,
    Resolving,
    CheckingOut,
    Verifying,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSample {
    pub job_id: JobId,
    pub repo_full_name: String,
    pub phase: JobPhase,
    pub indexed_objects: u32,
    pub received_objects: u32,
    pub total_objects: u32,
    pub received_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveJobProgress {
    pub job_id: JobId,
    pub repo_full_name: String,
    pub phase: JobPhase,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub percent: f32,
}

/// Overall progress snapshot. `started_at` is a `DateTime<Utc>` (not `Instant`)
/// so the type stays `Serialize`/`Deserialize` — useful for tests and snapshots.
/// The reducer will populate it from `chrono::Utc::now()` when the sync begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallProgress {
    pub total_jobs: usize,
    pub completed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub active: usize,
    pub bytes_transferred: u64,
    pub bytes_total_estimate: u64,
    pub percent: f32,
    pub throughput_bps: f64,
    pub eta: Option<std::time::Duration>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}
