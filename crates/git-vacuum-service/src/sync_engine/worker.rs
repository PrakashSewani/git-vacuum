use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use git_vacuum_core::{
    AppEvent, CloneStatus, JobId, JobSpec, LocalStatus, PlannedOperation, ProgressSample,
};
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::Services;
use crate::sync_engine::ProgressTracker;

pub struct Worker {
    pub services: Arc<Services>,
    pub job: JobSpec,
    pub progress_tx: mpsc::UnboundedSender<AppEvent>,
    pub app_tx: mpsc::UnboundedSender<AppEvent>,
    pub cancel_rx: watch::Receiver<bool>,
    pub pause_flag: Arc<std::sync::atomic::AtomicBool>,
    pub tracker: Arc<ProgressTracker>,
    pub result_tx: mpsc::Sender<JobOutcome>,
    pub run_id: Option<i64>,
}

pub enum JobOutcome {
    Success {
        job_id: JobId,
        repo_full_name: String,
        repo_id: Option<i64>,
        bytes: u64,
    },
    Updated {
        job_id: JobId,
        repo_full_name: String,
        repo_id: Option<i64>,
        bytes: u64,
        new_commits: i32,
    },
    UpToDate {
        job_id: JobId,
        repo_full_name: String,
        repo_id: Option<i64>,
    },
    Failed {
        job_id: JobId,
        repo_full_name: String,
        repo_id: Option<i64>,
        error: String,
    },
    Cancelled {
        job_id: JobId,
        repo_full_name: String,
        repo_id: Option<i64>,
    },
}

impl JobOutcome {
    pub fn repo_full_name(&self) -> &str {
        match self {
            JobOutcome::Success { repo_full_name, .. }
            | JobOutcome::Updated { repo_full_name, .. }
            | JobOutcome::UpToDate { repo_full_name, .. }
            | JobOutcome::Failed { repo_full_name, .. }
            | JobOutcome::Cancelled { repo_full_name, .. } => repo_full_name,
        }
    }
    pub fn job_id(&self) -> JobId {
        match self {
            JobOutcome::Success { job_id, .. }
            | JobOutcome::Updated { job_id, .. }
            | JobOutcome::UpToDate { job_id, .. }
            | JobOutcome::Failed { job_id, .. }
            | JobOutcome::Cancelled { job_id, .. } => *job_id,
        }
    }
    pub fn repo_id(&self) -> Option<i64> {
        match self {
            JobOutcome::Success { repo_id, .. }
            | JobOutcome::Updated { repo_id, .. }
            | JobOutcome::UpToDate { repo_id, .. }
            | JobOutcome::Failed { repo_id, .. }
            | JobOutcome::Cancelled { repo_id, .. } => *repo_id,
        }
    }
    pub fn bytes(&self) -> u64 {
        match self {
            JobOutcome::Success { bytes, .. } | JobOutcome::Updated { bytes, .. } => *bytes,
            _ => 0,
        }
    }
}

impl Worker {
    pub async fn run(self) {
        let job_id = self.job.job_id;
        let full_name = self.job.repo_full_name.clone();

        // Look up local repo id (best-effort)
        let repo_id = self.services.db.get_repo(self.job.repo_github_id).ok().flatten().map(|r| r.id);

        let outcome = match self.job.operation {
            PlannedOperation::Clone => self.run_clone().await,
            PlannedOperation::Sync => self.run_sync().await,
            PlannedOperation::Mirror => self.run_clone().await, // MVP: treat as clone
            PlannedOperation::Skip { .. } => {
                self.tracker.skipped.fetch_add(1, Ordering::Relaxed);
                JobOutcome::UpToDate { job_id, repo_full_name: full_name, repo_id }
            }
        };
        let _ = self.result_tx.send(outcome).await;
    }

    async fn run_clone(&self) -> JobOutcome {
        let job_id = self.job.job_id;
        let full_name = self.job.repo_full_name.clone();
        let path = self.job.local_path.clone();
        let url = self.job.clone_url.clone();
        let repo_id = self.services.db.get_repo(self.job.repo_github_id).ok().flatten().map(|r| r.id);

        self.tracker.register_active(crate::sync_engine::ActiveJob {
            job_id,
            repo_full_name: full_name.clone(),
            bytes_done: 0,
            bytes_total: 0,
            started_at: Instant::now(),
        });
        let _ = self.app_tx.send(AppEvent::SyncCloneStarted { job_id, repo_full_name: full_name.clone() });

        let on_progress = make_progress_cb(self.progress_tx.clone(), job_id, full_name.clone());
        let cancel = self.cancel_rx.clone();
        match self.services.git.clone_with_progress(&url, &path, on_progress, cancel).await {
            Ok(stats) => {
                self.tracker.cloned.fetch_add(1, Ordering::Relaxed);
                let _ = self.services.db.update_local_status(
                    self.job.repo_github_id,
                    &LocalStatus {
                        clone_status: CloneStatus::Cloned,
                        local_path: Some(path.to_string_lossy().to_string()),
                        local_size_kb: Some((stats.received_bytes / 1024) as i64),
                        last_synced_at: Some(chrono::Utc::now()),
                        last_error: None,
                        behind_count: 0,
                    },
                );
                JobOutcome::Success {
                    job_id,
                    repo_full_name: full_name,
                    repo_id,
                    bytes: stats.received_bytes,
                }
            }
            Err(e) => JobOutcome::Failed {
                job_id,
                repo_full_name: full_name,
                repo_id,
                error: format!("{e}"),
            },
        }
    }

    async fn run_sync(&self) -> JobOutcome {
        let job_id = self.job.job_id;
        let full_name = self.job.repo_full_name.clone();
        let path = self.job.local_path.clone();
        let repo_id = self.services.db.get_repo(self.job.repo_github_id).ok().flatten().map(|r| r.id);

        self.tracker.register_active(crate::sync_engine::ActiveJob {
            job_id,
            repo_full_name: full_name.clone(),
            bytes_done: 0,
            bytes_total: 0,
            started_at: Instant::now(),
        });
        let _ = self.app_tx.send(AppEvent::SyncFetchStarted { job_id, repo_full_name: full_name.clone() });

        let on_progress = make_progress_cb(self.progress_tx.clone(), job_id, full_name.clone());
        let cancel = self.cancel_rx.clone();
        match self.services.git.fetch(&path, on_progress, cancel).await {
            Ok(result) => {
                let _ = self.services.db.update_local_status(
                    self.job.repo_github_id,
                    &LocalStatus {
                        clone_status: CloneStatus::Cloned,
                        local_path: Some(path.to_string_lossy().to_string()),
                        local_size_kb: None,
                        last_synced_at: Some(chrono::Utc::now()),
                        last_error: None,
                        behind_count: result.behind_count,
                    },
                );
                if result.new_commits == 0 && result.behind_count == 0 {
                    JobOutcome::UpToDate { job_id, repo_full_name: full_name, repo_id }
                } else {
                    JobOutcome::Updated {
                        job_id,
                        repo_full_name: full_name,
                        repo_id,
                        bytes: result.bytes_fetched,
                        new_commits: result.new_commits as i32,
                    }
                }
            }
            Err(e) => JobOutcome::Failed {
                job_id,
                repo_full_name: full_name,
                repo_id,
                error: format!("{e}"),
            },
        }
    }
}

fn make_progress_cb(
    tx: mpsc::UnboundedSender<AppEvent>,
    job_id: JobId,
    full_name: String,
) -> Box<dyn Fn(ProgressSample) + Send + Sync> {
    let last_emit = std::sync::Mutex::new(None::<Instant>);
    Box::new(move |sample: ProgressSample| {
        let now = Instant::now();
        let mut guard = last_emit.lock().unwrap();
        let should_emit = guard
            .map(|t| now.duration_since(t) >= std::time::Duration::from_millis(100))
            .unwrap_or(true);
        if should_emit {
            let _ = tx.send(AppEvent::SyncCloneProgress {
                job_id,
                repo_full_name: full_name.clone(),
                bytes: sample.received_bytes,
                total: sample.total_objects as u64,
            });
            *guard = Some(now);
        }
    })
}

#[allow(dead_code)]
pub fn path_for_job(base: &PathBuf, job: &JobSpec) -> PathBuf {
    job.local_path.clone()
}
