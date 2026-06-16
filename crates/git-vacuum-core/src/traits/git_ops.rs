use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;

use crate::types::job::ProgressSample;



#[derive(Debug, Clone)]
pub struct CloneStats {
    pub received_bytes: u64,
    pub total_objects: usize,
    pub indexed_objects: usize,
    pub cancelled: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub new_commits: u32,
    pub bytes_fetched: u64,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct LocalRepoStatus {
    pub behind_count: i32,
    pub ahead_count: i32,
    pub is_dirty: bool,
    pub current_branch: String,
}

#[async_trait]
pub trait GitOps: Send + Sync {
    async fn clone_repo(
        &self,
        url: &str,
        path: &Path,
        progress_tx: tokio::sync::mpsc::UnboundedSender<ProgressSample>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<CloneStats, String>;

    async fn fetch(
        &self,
        path: &Path,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<FetchResult, String>;

    async fn status(&self, path: &Path) -> Result<LocalRepoStatus, String>;

    fn is_git_repo(&self, path: &Path) -> bool;

    async fn mirror_clone(
        &self,
        url: &str,
        path: &Path,
        progress_tx: tokio::sync::mpsc::UnboundedSender<ProgressSample>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<CloneStats, String>;
}
