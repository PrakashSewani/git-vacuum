use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;

use crate::types::progress::ProgressSample;

#[derive(Debug, Clone, Default)]
pub struct CloneStats {
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub received_objects: u32,
    pub total_objects: u32,
    pub duration: Duration,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FetchResult {
    pub new_commits: u32,
    pub bytes_fetched: u64,
    pub duration: Duration,
    pub behind_count: i32,
    pub ahead_count: i32,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LocalRepoStatus {
    pub behind_count: i32,
    pub ahead_count: i32,
    pub clean: bool,
    pub size_kb: u64,
}

#[async_trait]
pub trait GitOps: Send + Sync {
    async fn clone_with_progress(
        &self,
        url: &str,
        path: &Path,
        on_progress: Box<dyn Fn(ProgressSample) + Send + Sync>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<CloneStats, crate::error::GitError>;

    async fn fetch(
        &self,
        path: &Path,
        on_progress: Box<dyn Fn(ProgressSample) + Send + Sync>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<FetchResult, crate::error::GitError>;

    async fn local_status(&self, path: &Path) -> Result<LocalRepoStatus, crate::error::GitError>;

    fn is_git_repo(&self, path: &Path) -> bool;

    fn remove_repo(&self, path: &Path) -> Result<(), crate::error::GitError>;
}
