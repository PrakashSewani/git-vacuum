use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{mpsc, Semaphore, watch};

use git_vacuum_core::AppEvent;
use git_vacuum_core::traits::GitOps;
use git_vacuum_core::types::job::{JobSpec, PlannedOperation, Priority};
use git_vacuum_core::types::repo::RepoEntry;
use git_vacuum_core::types::sync::{JobOutcomeSummary, JobSummary, SyncOptions, SyncSummary};

pub async fn run_sync(
    repos: Vec<RepoEntry>,
    base_path: PathBuf,
    options: SyncOptions,
    _db: Arc<dyn git_vacuum_core::traits::Database>,
    git: Arc<dyn GitOps>,
    progress_tx: mpsc::UnboundedSender<AppEvent>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
    cancel_rx: watch::Receiver<bool>,
) -> Result<SyncSummary, String> {
    let now = chrono::Utc::now();
    let started_dt = now;
    let run_id = started_dt.timestamp_millis();

    let jobs = build_jobs(&repos, &base_path, &options);
    let total = jobs.len();

    let _ = app_tx.send(AppEvent::SyncAllStarted { run_id, total_jobs: total });

    let semaphore = Arc::new(Semaphore::new(options.concurrency));
    let pause_flag = Arc::new(AtomicBool::new(false));
    let (result_tx, mut result_rx) = mpsc::channel(options.concurrency * 2);

    let _progress_tx = progress_tx.clone();
    let _cancel_rx = cancel_rx.clone();
    let _pause_flag = pause_flag.clone();
    let _dispatcher_handle = tokio::spawn(async move {
        run_dispatcher(
            jobs, semaphore, _pause_flag,
            git, _progress_tx, result_tx, _cancel_rx,
        ).await
    });

    let mut completed = Vec::new();

    while let Some(outcome) = result_rx.recv().await {
        completed.push(outcome);
    }

    let (cloned, updated, skipped, failed, cancelled) = count_outcomes(&completed);
    let bytes = total_bytes(&completed);

    let summary = SyncSummary {
        run_id,
        started_at: started_dt,
        completed_at: chrono::Utc::now(),
        total_repos: total,
        cloned_count: cloned,
        updated_count: updated,
        skipped_count: skipped,
        failed_count: failed,
        cancelled_count: cancelled,
        bytes_transferred: bytes,
        duration: Duration::default(),
        jobs: Vec::new(),
    };

    let _ = app_tx.send(AppEvent::SyncAllCompleted { summary: summary.clone() });
    Ok(summary)
}

fn build_jobs(repos: &[RepoEntry], base_path: &PathBuf, options: &SyncOptions) -> Vec<JobSpec> {
    let mut jobs = Vec::new();
    for (idx, repo) in repos.iter().enumerate() {
        let local_path = base_path.join(&repo.owner_login).join(&repo.name);
        let clone_url = match &options.protocol {
            git_vacuum_core::CloneProtocol::Ssh =>
                repo.clone_url_ssh.clone().unwrap_or_else(|| repo.clone_url_https.clone()),
            git_vacuum_core::CloneProtocol::Https { .. } =>
                repo.clone_url_https.clone(),
        };

        let operation = if options.mirror {
            PlannedOperation::Mirror
        } else if local_path.exists() {
            PlannedOperation::Sync
        } else {
            PlannedOperation::Clone
        };

        let priority = match repo.size_kb {
            Some(sz) if sz < 1024 => Priority::High,
            Some(sz) if sz > 102400 => Priority::Low,
            _ => Priority::Normal,
        };

        jobs.push(JobSpec {
            job_id: idx as u64,
            repo_full_name: repo.full_name.clone(),
            repo_github_id: repo.github_id,
            owner_login: repo.owner_login.clone(),
            clone_url,
            local_path,
            operation,
            priority,
            attempt: 0,
        });
    }
    jobs.sort_by_key(|j| j.priority);
    jobs
}

async fn run_dispatcher(
    mut jobs: Vec<JobSpec>,
    semaphore: Arc<Semaphore>,
    pause_flag: Arc<AtomicBool>,
    git: Arc<dyn GitOps>,
    progress_tx: mpsc::UnboundedSender<AppEvent>,
    result_tx: mpsc::Sender<JobOutcome>,
    cancel_rx: watch::Receiver<bool>,
) {
    while !jobs.is_empty() {
        if *cancel_rx.borrow() || pause_flag.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };

        let job = jobs.remove(0);
        let git = git.clone();
        let progress_tx = progress_tx.clone();
        let result_tx = result_tx.clone();
        let cancel_rx = cancel_rx.clone();

        tokio::spawn(async move {
            let outcome = execute_job(job, git, &progress_tx, &cancel_rx).await;
            let _ = result_tx.send(outcome).await;
            drop(permit);
        });
    }
}

async fn execute_job(
    job: JobSpec,
    git: Arc<dyn GitOps>,
    progress_tx: &mpsc::UnboundedSender<AppEvent>,
    cancel_rx: &watch::Receiver<bool>,
) -> JobOutcome {
    if *cancel_rx.borrow() {
        return JobOutcome::Cancelled;
    }

    if let Some(parent) = job.local_path.parent() {
        if let Err(_e) = std::fs::create_dir_all(parent) {
            return JobOutcome::Failed;
        }
    }

    match &job.operation {
        PlannedOperation::Clone | PlannedOperation::Mirror => {
            let (prog_tx, _) = mpsc::unbounded_channel();
            let _ = progress_tx.send(AppEvent::SyncCloneStarted {
                repo_full_name: job.repo_full_name.clone(), job_id: job.job_id,
            });

            match git.clone_repo(&job.clone_url, &job.local_path, prog_tx, cancel_rx.clone()).await {
                Ok(stats) if stats.cancelled => JobOutcome::Cancelled,
                Ok(stats) => {
                    let _ = progress_tx.send(AppEvent::SyncCloneCompleted {
                        repo_full_name: job.repo_full_name.clone(), job_id: job.job_id,
                        size_bytes: stats.received_bytes, duration: stats.duration,
                    });
                    JobOutcome::Success { bytes: stats.received_bytes }
                }
                Err(_) => JobOutcome::Failed,
            }
        }
        PlannedOperation::Sync => {
            let _ = progress_tx.send(AppEvent::SyncFetchStarted {
                repo_full_name: job.repo_full_name.clone(), job_id: job.job_id,
            });

            match git.fetch(&job.local_path, cancel_rx.clone()).await {
                Ok(fetch_result) => {
                    let _ = progress_tx.send(AppEvent::SyncFetchCompleted {
                        repo_full_name: job.repo_full_name.clone(), job_id: job.job_id,
                        result: fetch_result.clone(),
                    });
                    if fetch_result.new_commits == 0 {
                        let _ = progress_tx.send(AppEvent::SyncRepoUpToDate {
                            repo_full_name: job.repo_full_name.clone(), job_id: job.job_id,
                        });
                    }
                    JobOutcome::Success { bytes: fetch_result.bytes_fetched }
                }
                Err(_) => JobOutcome::Failed,
            }
        }
        PlannedOperation::Skip { .. } => JobOutcome::Skipped,
    }
}

#[derive(Debug, Clone)]
enum JobOutcome {
    Success { bytes: u64 },
    Failed,
    Skipped,
    Cancelled,
}

fn count_outcomes(completed: &[JobOutcome]) -> (usize, usize, usize, usize, usize) {
    let (mut cloned, mut updated, mut skipped, mut failed, mut cancelled) = (0, 0, 0, 0, 0);
    for outcome in completed {
        match outcome {
            JobOutcome::Success { .. } => cloned += 1,
            JobOutcome::Failed => failed += 1,
            JobOutcome::Skipped => skipped += 1,
            JobOutcome::Cancelled => cancelled += 1,
        }
    }
    (cloned, updated, skipped, failed, cancelled)
}

fn total_bytes(completed: &[JobOutcome]) -> u64 {
    completed.iter().map(|o| match o {
        JobOutcome::Success { bytes } => *bytes,
        _ => 0,
    }).sum()
}
