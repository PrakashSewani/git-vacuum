use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use git2::{FetchOptions, RemoteCallbacks, Repository};
use git_vacuum_core::{
    CloneStats, FetchResult, GitError, GitOps, JobId, JobPhase, LocalRepoStatus, ProgressSample,
};
use parking_lot::Mutex;

pub struct Git2GitOps;

impl Git2GitOps {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Git2GitOps {
    fn default() -> Self {
        Self::new()
    }
}

/// Orphan-rule workaround: `git2::Error` lives in `git2`, `GitError` lives in
/// `git-vacuum-core`. We can't `impl From<git2::Error> for GitError` here, so
/// we expose this mapping function and call it explicitly at every conversion
/// site. (Same pattern as `SqliteErr` in `git-vacuum-db`.)
pub(crate) fn map_git_err(e: git2::Error) -> GitError {
    match e.code() {
        git2::ErrorCode::NotFound => GitError::NotFound,
        git2::ErrorCode::Auth => GitError::AuthRequired,
        _ => GitError::Git2(e.message().to_string()),
    }
}

#[allow(clippy::type_complexity)]
fn build_callback(
    on_progress: Arc<Mutex<Option<Box<dyn Fn(ProgressSample) + Send + Sync>>>>,
) -> impl FnMut(git2::Progress<'_>) -> bool + Send + 'static {
    let mut last_emit: Option<Instant> = None;
    move |p: git2::Progress<'_>| {
        let now = Instant::now();
        let should_emit = last_emit
            .map(|t| now.duration_since(t) >= Duration::from_millis(100))
            .unwrap_or(true);
        if should_emit {
            if let Some(cb) = on_progress.lock().as_ref() {
                let sample = ProgressSample {
                    job_id: JobId(0),
                    repo_full_name: String::new(),
                    phase: JobPhase::Receiving,
                    indexed_objects: p.indexed_objects() as u32,
                    received_objects: p.received_objects() as u32,
                    total_objects: p.total_objects() as u32,
                    received_bytes: p.received_bytes() as u64,
                };
                cb(sample);
            }
            last_emit = Some(now);
        }
        true
    }
}

#[async_trait]
impl GitOps for Git2GitOps {
    async fn clone_with_progress(
        &self,
        url: &str,
        path: &Path,
        on_progress: Box<dyn Fn(ProgressSample) + Send + Sync>,
        mut cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<CloneStats, GitError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| GitError::Internal(format!("create_dir_all: {e}")))?;
        }

        let url_owned = url.to_string();
        let path_owned = path.to_path_buf();
        let cb_slot: Arc<Mutex<Option<Box<dyn Fn(ProgressSample) + Send + Sync>>>> =
            Arc::new(Mutex::new(Some(on_progress)));

        let result = tokio::task::spawn_blocking(move || -> Result<CloneStats, git2::Error> {
            let started = Instant::now();
            let mut opts = git2::build::RepoBuilder::new();
            let mut fetch_opts = FetchOptions::new();
            let mut callbacks = RemoteCallbacks::new();
            let cb = build_callback(cb_slot);
            callbacks.transfer_progress(cb);
            fetch_opts.remote_callbacks(callbacks);
            opts.fetch_options(fetch_opts);
            let repo = opts.clone(&url_owned, &path_owned)?;
            // Reset HEAD to match default branch from the clone.
            if let Ok(head_ref) = repo.head() {
                if let Some(target) = head_ref.target() {
                    let _ = repo.set_head_detached(target);
                }
            }
            Ok(CloneStats {
                received_bytes: 0,
                total_bytes: 0,
                received_objects: 0,
                total_objects: 0,
                duration: started.elapsed(),
                cancelled: false,
            })
        })
        .await
        .map_err(|e| GitError::Internal(format!("join: {e}")))?;

        if *cancel.borrow_and_update() {
            return Ok(CloneStats {
                cancelled: true,
                ..Default::default()
            });
        }
        result.map_err(map_git_err)
    }

    async fn fetch(
        &self,
        path: &Path,
        on_progress: Box<dyn Fn(ProgressSample) + Send + Sync>,
        mut cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<FetchResult, GitError> {
        let path_owned = path.to_path_buf();
        let cb_slot: Arc<Mutex<Option<Box<dyn Fn(ProgressSample) + Send + Sync>>>> =
            Arc::new(Mutex::new(Some(on_progress)));

        let result = tokio::task::spawn_blocking(move || -> Result<FetchResult, git2::Error> {
            let started = Instant::now();
            let repo = Repository::open(&path_owned)?;
            let mut remote = repo.find_remote("origin")?;
            let mut fetch_opts = FetchOptions::new();
            let mut callbacks = RemoteCallbacks::new();
            let cb = build_callback(cb_slot);
            callbacks.transfer_progress(cb);
            fetch_opts.remote_callbacks(callbacks);
            remote.fetch(&["refs/heads/*:refs/heads/*"], Some(&mut fetch_opts), None)?;
            let stats = remote.stats();
            let fetch_head = repo
                .find_reference("FETCH_HEAD")
                .ok()
                .and_then(|r| r.target());
            let head_oid = repo.head()?.target();
            let new_commits = match (head_oid, fetch_head) {
                (Some(h), Some(f)) if h != f => {
                    repo.graph_ahead_behind(h, f).map(|(_, b)| b).unwrap_or(0)
                }
                _ => 0,
            };
            Ok(FetchResult {
                new_commits: new_commits as u32,
                bytes_fetched: stats.received_bytes() as u64,
                duration: started.elapsed(),
                behind_count: new_commits as i32,
                ahead_count: 0,
                cancelled: false,
            })
        })
        .await
        .map_err(|e| GitError::Internal(format!("join: {e}")))?;

        if *cancel.borrow_and_update() {
            return Ok(FetchResult {
                cancelled: true,
                ..Default::default()
            });
        }
        result.map_err(map_git_err)
    }

    async fn local_status(&self, path: &Path) -> Result<LocalRepoStatus, GitError> {
        let path_owned = path.to_path_buf();
        let res = tokio::task::spawn_blocking(move || -> Result<LocalRepoStatus, git2::Error> {
            let repo = Repository::open(&path_owned)?;
            let head = repo.head()?.target();
            let local = head.unwrap_or(git2::Oid::zero());
            let mut behind = 0i32;
            let mut ahead = 0i32;
            if let Ok(remote_branch) = repo.find_reference("refs/remotes/origin/HEAD") {
                if let Some(remote_oid) = remote_branch.target() {
                    let (a, b) = repo.graph_ahead_behind(local, remote_oid)?;
                    ahead = a as i32;
                    behind = b as i32;
                }
            }
            let status = repo.statuses(Some(
                &mut git2::StatusOptions::new()
                    .include_untracked(false)
                    .include_unmodified(false),
            ))?;
            let clean = status.is_empty();
            let size_kb = dir_size_kb(&path_owned).unwrap_or(0);
            Ok(LocalRepoStatus {
                behind_count: behind,
                ahead_count: ahead,
                clean,
                size_kb,
            })
        })
        .await
        .map_err(|e| GitError::Internal(format!("join: {e}")))?;
        res.map_err(map_git_err)
    }

    fn is_git_repo(&self, path: &Path) -> bool {
        Repository::open(path).is_ok()
    }

    fn remove_repo(&self, path: &Path) -> Result<(), GitError> {
        if path.exists() {
            std::fs::remove_dir_all(path)
                .map_err(|e| GitError::Internal(format!("remove_dir_all: {e}")))?;
        }
        Ok(())
    }
}

fn dir_size_kb(path: &Path) -> std::io::Result<u64> {
    let mut total: u64 = 0;
    if !path.is_dir() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().map(|n| n == ".git").unwrap_or(false) {
                // Count .git as 1 byte (proxy for "this is a git repo")
            } else {
                total += dir_size_kb(&p)?;
            }
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    Ok(total / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_id() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    fn make_bare_repo() -> PathBuf {
        // Create a normal repo, add a file, commit, then make a bare clone of it.
        let work = std::env::temp_dir().join(format!(
            "gv-test-work-{}-{}",
            std::process::id(),
            unique_id()
        ));
        let bare = std::env::temp_dir().join(format!(
            "gv-test-bare-{}-{}",
            std::process::id(),
            unique_id()
        ));
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bare);
        std::fs::create_dir_all(&work).unwrap();

        let repo = Repository::init(&work).unwrap();
        std::fs::write(work.join("hello.txt"), b"hello world").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("hello.txt")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        // Now clone bare
        let mut opts = git2::build::RepoBuilder::new();
        opts.bare(true);
        let _bare_repo = opts.clone(work.to_str().unwrap(), &bare).unwrap();
        let _ = std::fs::remove_dir_all(&work);
        bare
    }

    #[tokio::test]
    async fn is_git_repo_works() {
        let ops = Git2GitOps::new();
        let bare = make_bare_repo();
        assert!(ops.is_git_repo(&bare));
        let not_a_repo = std::env::temp_dir();
        assert!(!ops.is_git_repo(&not_a_repo.join(format!("not-a-repo-{}", unique_id()))));
        let _ = std::fs::remove_dir_all(&bare);
    }

    #[tokio::test]
    async fn clone_progress_works() {
        let bare = make_bare_repo();
        let dest = std::env::temp_dir().join(format!(
            "gv-test-clone-{}-{}",
            std::process::id(),
            unique_id()
        ));
        let _ = std::fs::remove_dir_all(&dest);
        let ops = Git2GitOps::new();
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let stats = ops
            .clone_with_progress(bare.to_str().unwrap(), &dest, Box::new(|_sample| {}), rx)
            .await;
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_dir_all(&dest);
        assert!(stats.is_ok(), "clone failed: {:?}", stats);
    }
}
