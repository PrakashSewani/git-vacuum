use std::path::PathBuf;
use std::time::Instant;

pub type JobId = u64;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum PlannedOperation {
    Clone,
    Sync,
    Mirror,
    Skip { reason: SkipReason },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkipReason {
    AlreadyUpToDate,
    LocalOnly,
    NoAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    High = 0,
    Normal = 1,
    Low = 2,
}

#[derive(Debug, Clone)]
pub struct RunningJob {
    pub spec: JobSpec,
    pub started_at: Instant,
    pub progress: JobProgress,
}

impl RunningJob {
    pub fn new(spec: JobSpec) -> Self {
        Self {
            spec,
            started_at: Instant::now(),
            progress: JobProgress::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobProgress {
    pub phase: JobPhase,
    pub received_objects: u32,
    pub total_objects: u32,
    pub received_bytes: usize,
    pub percent: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobPhase {
    Queued,
    Connecting,
    Receiving,
    Resolving,
    CheckingOut,
    Verifying,
}

impl Default for JobPhase {
    fn default() -> Self {
        JobPhase::Queued
    }
}

#[derive(Debug, Clone)]
pub struct ProgressSample {
    pub job_id: JobId,
    pub repo_full_name: String,
    pub phase: JobPhase,
    pub indexed_objects: u32,
    pub received_objects: u32,
    pub total_objects: u32,
    pub received_bytes: usize,
}
