use std::path::Path;
use std::time::{Duration, Instant};

use git2::FetchOptions;
use git_vacuum_core::traits::git_ops::{CloneStats, FetchResult, GitOps, LocalRepoStatus};
use git_vacuum_core::types::job::{JobPhase, ProgressSample};
use tokio::sync::mpsc;
use tokio::sync::watch;

pub struct Git2Ops;

impl Git2Ops {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl GitOps for Git2Ops {
    async fn clone_repo(
        &self,
        url: &str,
        path: &Path,
        progress_tx: mpsc::UnboundedSender<ProgressSample>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<CloneStats, String> {
        let url = url.to_string();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || clone_impl(&url, &path, &progress_tx, &cancel_rx))
            .await
            .map_err(|e| format!("Join error: {}", e))?
    }

    async fn fetch(
        &self,
        path: &Path,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<FetchResult, String> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || fetch_impl(&path, &cancel_rx))
            .await
            .map_err(|e| format!("Join error: {}", e))?
    }

    async fn status(&self, path: &Path) -> Result<LocalRepoStatus, String> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || status_impl(&path))
            .await
            .map_err(|e| format!("Join error: {}", e))?
    }

    fn is_git_repo(&self, path: &Path) -> bool {
        git2::Repository::open(path).is_ok()
    }

    async fn mirror_clone(
        &self,
        url: &str,
        path: &Path,
        progress_tx: mpsc::UnboundedSender<ProgressSample>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<CloneStats, String> {
        let url = url.to_string();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || mirror_clone_impl(&url, &path, &progress_tx, &cancel_rx))
            .await
            .map_err(|e| format!("Join error: {}", e))?
    }
}

fn clone_impl(
    url: &str,
    path: &Path,
    progress_tx: &mpsc::UnboundedSender<ProgressSample>,
    cancel_rx: &watch::Receiver<bool>,
) -> Result<CloneStats, String> {
    let start = Instant::now();
    if *cancel_rx.borrow() {
        return Ok(CloneStats {
            received_bytes: 0, total_objects: 0, indexed_objects: 0,
            cancelled: true, duration: start.elapsed(),
        });
    }

    let mut cb = git2::RemoteCallbacks::new();
    let tx = progress_tx.clone();
    let cancel = cancel_rx.clone();
    cb.transfer_progress(move |p| {
        let _ = tx.send(ProgressSample {
            job_id: 0, repo_full_name: String::new(),
            phase: JobPhase::Receiving,
            indexed_objects: p.indexed_objects() as u32,
            received_objects: p.received_objects() as u32,
            total_objects: p.total_objects() as u32,
            received_bytes: p.received_bytes(),
        });
        !*cancel.borrow()
    });

    let mut opts = FetchOptions::new();
    opts.remote_callbacks(cb);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(opts);

    builder.clone(url, path).map_err(|e| format!("Clone failed: {}", e))?;
    Ok(CloneStats {
        received_bytes: 0, total_objects: 0, indexed_objects: 0,
        cancelled: false, duration: start.elapsed(),
    })
}

fn fetch_impl(
    path: &Path,
    cancel_rx: &watch::Receiver<bool>,
) -> Result<FetchResult, String> {
    let start = Instant::now();
    if *cancel_rx.borrow() {
        return Ok(FetchResult { new_commits: 0, bytes_fetched: 0, duration: start.elapsed() });
    }

    let repo = git2::Repository::open(path).map_err(|e| format!("Cannot open repo: {}", e))?;
    let mut remote = repo.find_remote("origin").map_err(|e| format!("No origin: {}", e))?;

    let cancel = cancel_rx.clone();
    let mut cb = git2::RemoteCallbacks::new();
    cb.transfer_progress(move |_p| !*cancel.borrow());

    let mut opts = FetchOptions::new();
    opts.remote_callbacks(cb);

    remote.fetch(&["refs/heads/*:refs/remotes/origin/*"], Some(&mut opts), None)
        .map_err(|e| format!("Fetch failed: {}", e))?;

    Ok(FetchResult { new_commits: 0, bytes_fetched: 0, duration: start.elapsed() })
}

fn status_impl(path: &Path) -> Result<LocalRepoStatus, String> {
    let repo = git2::Repository::open(path).map_err(|e| format!("Cannot open repo: {}", e))?;
    let branch = repo.head().ok().and_then(|h| h.shorthand().map(|s| s.to_string())).unwrap_or_else(|| "unknown".to_string());
    let dirty = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);
    Ok(LocalRepoStatus { behind_count: 0, ahead_count: 0, is_dirty: dirty, current_branch: branch })
}

fn mirror_clone_impl(
    url: &str,
    path: &Path,
    _progress_tx: &mpsc::UnboundedSender<ProgressSample>,
    cancel_rx: &watch::Receiver<bool>,
) -> Result<CloneStats, String> {
    let start = Instant::now();
    if *cancel_rx.borrow() {
        return Ok(CloneStats {
            received_bytes: 0, total_objects: 0, indexed_objects: 0,
            cancelled: true, duration: start.elapsed(),
        });
    }

    let cancel = cancel_rx.clone();
    let mut cb = git2::RemoteCallbacks::new();
    cb.transfer_progress(move |_p| !*cancel.borrow());

    let mut opts = FetchOptions::new();
    opts.remote_callbacks(cb);

    let mut builder = git2::build::RepoBuilder::new();
    builder.bare(true);
    builder.fetch_options(opts);

    builder.clone(url, path).map_err(|e| format!("Mirror clone failed: {}", e))?;
    Ok(CloneStats {
        received_bytes: 0, total_objects: 0, indexed_objects: 0,
        cancelled: false, duration: start.elapsed(),
    })
}
