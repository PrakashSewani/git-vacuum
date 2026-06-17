use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use git_vacuum_core::{
    AppEvent, CloneStatus, JobId, JobSpec, LocalStatus, PlannedOperation, Priority,
    RepoEntry, SyncOptions, SyncSummary,
};
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::Services;

mod plan;
mod worker;

/// Top-level sync pipeline (4 stages).
/// - Stage 1: resolve plan (Clone vs Sync vs Skip per repo)
/// - Stage 2: worker pool (bounded by `concurrency` semaphore)
/// - Stage 3: result collector (aggregates outcomes, persists to DB)
/// - Stage 4: emit final summary
pub async fn run(
    services: Arc<Services>,
    request: super::SyncRequest,
    progress_tx: mpsc::UnboundedSender<AppEvent>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
    cancel_rx: watch::Receiver<bool>,
) -> SyncSummary {
    let started_at = Instant::now();
    let total_jobs = request.repos.len();
    let pause_flag = Arc::new(AtomicBool::new(false));

    // Stage 1: resolve plan
    let jobs = plan::resolve_plan(&services, &request).await;
    let effective_total = jobs.len();

    // Progress tracker
    let tracker = Arc::new(ProgressTracker::new(effective_total));

    // Semaphore-bounded channel for results
    let (result_tx, mut result_rx) = mpsc::channel(effective_total.max(1) * 2);

    // Insert a sync_runs row
    let run_id = services
        .db
        .insert_sync_run(&git_vacuum_core::NewSyncRun {
            started_at: Utc::now(),
            trigger: "manual".into(),
            total_repos: total_jobs as i32,
            options_json: None,
        })
        .ok();

    // Stage 2: spawn workers
    let concurrency = request.concurrency.max(1);
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut worker_handles = Vec::with_capacity(effective_total);

    for job in jobs {
        // Honor pause/cancel before grabbing a permit
        if *cancel_rx.borrow() {
            break;
        }
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };

        let worker = worker::Worker {
            services: services.clone(),
            job,
            progress_tx: progress_tx.clone(),
            app_tx: app_tx.clone(),
            cancel_rx: cancel_rx.clone(),
            pause_flag: pause_flag.clone(),
            tracker: tracker.clone(),
            result_tx: result_tx.clone(),
            run_id,
        };
        let handle = tokio::spawn(async move {
            let _permit = permit; // released on drop
            worker.run().await;
        });
        worker_handles.push(handle);
    }

    // Stage 3: collect results (runs concurrently with workers)
    let collector = Collector {
        services: services.clone(),
        run_id,
        tracker: tracker.clone(),
        app_tx: app_tx.clone(),
    };
    let collect_handle = tokio::spawn(async move {
        collector.run(&mut result_rx).await;
    });

    // Wait for workers to finish (or cancel)
    for h in worker_handles {
        let _ = h.await;
    }
    // Signal the collector that no more results are coming
    drop(result_tx);
    let _ = collect_handle.await;

    let summary = SyncSummary {
        total_jobs: total_jobs,
        cloned: tracker.cloned.load(Ordering::Relaxed),
        updated: tracker.updated.load(Ordering::Relaxed),
        up_to_date: tracker.up_to_date.load(Ordering::Relaxed),
        failed: tracker.failed.load(Ordering::Relaxed),
        skipped: tracker.skipped.load(Ordering::Relaxed),
        cancelled: tracker.cancelled.load(Ordering::Relaxed),
        bytes_transferred: tracker.bytes_transferred.load(Ordering::Relaxed),
        duration: started_at.elapsed(),
    };

    // Update sync_runs row
    if let Some(id) = run_id {
        let _ = services.db.update_sync_run(
            id,
            &git_vacuum_core::SyncRunUpdate {
                completed_at: Some(Utc::now()),
                status: Some(if summary.failed == 0 && summary.cancelled == 0 {
                    "completed".to_string()
                } else if summary.cancelled > 0 {
                    "cancelled".to_string()
                } else {
                    "completed".to_string()
                }),
                cloned_count: Some(summary.cloned as i32),
                updated_count: Some(summary.updated as i32),
                failed_count: Some(summary.failed as i32),
                bytes_transferred: Some(summary.bytes_transferred as i64),
            },
        );
    }

    let _ = app_tx.send(AppEvent::SyncAllCompleted { summary: summary.clone() });
    summary
}

pub struct ProgressTracker {
    pub total_jobs: usize,
    pub completed: AtomicUsize,
    pub succeeded: AtomicUsize,
    pub failed: AtomicUsize,
    pub skipped: AtomicUsize,
    pub updated: AtomicUsize,
    pub up_to_date: AtomicUsize,
    pub cloned: AtomicUsize,
    pub cancelled: AtomicUsize,
    pub bytes_transferred: AtomicU64,
    pub started_at: Instant,
    pub active: parking_lot::Mutex<HashMap<JobId, ActiveJob>>,
}

#[derive(Debug, Clone)]
pub struct ActiveJob {
    pub job_id: JobId,
    pub repo_full_name: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub started_at: Instant,
}

impl ProgressTracker {
    pub fn new(total: usize) -> Self {
        Self {
            total_jobs: total,
            completed: AtomicUsize::new(0),
            succeeded: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            updated: AtomicUsize::new(0),
            up_to_date: AtomicUsize::new(0),
            cloned: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            bytes_transferred: AtomicU64::new(0),
            started_at: Instant::now(),
            active: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    pub fn register_active(&self, job: ActiveJob) {
        self.active.lock().insert(job.job_id, job);
    }

    pub fn update_progress(&self, job_id: JobId, bytes: u64) {
        if let Some(j) = self.active.lock().get_mut(&job_id) {
            j.bytes_done = bytes;
        }
    }

    pub fn complete_active(&self, job_id: JobId, success: bool, bytes: u64) {
        self.active.lock().remove(&job_id);
        self.completed.fetch_add(1, Ordering::Relaxed);
        if success {
            self.succeeded.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed.fetch_add(1, Ordering::Relaxed);
        }
        self.bytes_transferred.fetch_add(bytes, Ordering::Relaxed);
    }
}

pub struct Collector {
    pub services: Arc<Services>,
    pub run_id: Option<i64>,
    pub tracker: Arc<ProgressTracker>,
    pub app_tx: mpsc::UnboundedSender<AppEvent>,
}

impl Collector {
    async fn run(&self, result_rx: &mut mpsc::Receiver<worker::JobOutcome>) {
        while let Some(outcome) = result_rx.recv().await {
            self.process(outcome).await;
        }
    }

    async fn process(&self, outcome: worker::JobOutcome) {
        use worker::JobOutcome::*;
        let repo_full_name: String = outcome.repo_full_name().to_string();
        let job_id = outcome.job_id();

        match &outcome {
            Success { bytes, .. } => {
                self.tracker.complete_active(job_id, true, *bytes);
                let _ = self.app_tx.send(AppEvent::SyncCloneCompleted {
                    job_id,
                    repo_full_name: repo_full_name.clone(),
                    size_bytes: *bytes,
                    duration: Duration::from_secs(0),
                });
            }
            Updated { bytes, new_commits, .. } => {
                self.tracker.complete_active(job_id, true, *bytes);
                self.tracker.updated.fetch_add(1, Ordering::Relaxed);
                let _ = self.app_tx.send(AppEvent::SyncFetchCompleted {
                    job_id,
                    repo_full_name: repo_full_name.clone(),
                    new_commits: *new_commits as u32,
                    bytes_fetched: *bytes,
                    duration: Duration::from_secs(0),
                });
            }
            UpToDate { .. } => {
                self.tracker.complete_active(job_id, true, 0);
                self.tracker.up_to_date.fetch_add(1, Ordering::Relaxed);
                let _ = self.app_tx.send(AppEvent::SyncRepoUpToDate {
                    job_id,
                    repo_full_name: repo_full_name.clone(),
                });
            }
            Failed { error, .. } => {
                self.tracker.complete_active(job_id, false, 0);
                let _ = self.app_tx.send(AppEvent::SyncRepoFailed {
                    job_id,
                    repo_full_name: repo_full_name.clone(),
                    error: error.clone(),
                });
            }
            Cancelled { .. } => {
                self.tracker.cancelled.fetch_add(1, Ordering::Relaxed);
                self.tracker.active.lock().remove(&job_id);
            }
        }

        // Persist to DB (best-effort; never panic)
        if let (Some(rid), Some(repo)) = (self.run_id, outcome.repo_id()) {
            let (operation, status) = match &outcome {
                Success { .. } => ("clone", "success"),
                Updated { .. } => ("sync", "success"),
                UpToDate { .. } => ("sync", "success"),
                Failed { .. } => ("clone", "failed"),
                Cancelled { .. } => ("clone", "failed"),
            };
            let bytes = outcome.bytes();
            let error = match &outcome {
                Failed { error, .. } => Some(error.clone()),
                _ => None,
            };
            let _ = self.services.db.insert_sync_entries(&[git_vacuum_core::NewSyncEntry {
                run_id: rid,
                repo_id: repo,
                operation: operation.to_string(),
                status: status.to_string(),
                bytes_transferred: bytes as i64,
                new_commits: 0,
                duration_ms: None,
                error_message: error,
            }]);
        }
    }
}
