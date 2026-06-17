//! Service orchestration layer.
//!
//! This crate implements the orchestration that ties together the four
//! infrastructure traits (`Database`, `GithubApi`, `GitOps`, `KeyringStore`).
//! It depends ONLY on `git-vacuum-core` for types/traits — never on the
//! concrete infrastructure crates. This is the hexagonal boundary.
//!
//! ## Sub-modules
//! - `sync_engine` — multi-stage clone/sync pipeline (4 stages, semaphore-bounded)
//! - `discovery` — fetch remote repos, merge with local cache, persist
//! - `auth_service` — PAT + OAuth device flow + keyring write
//! - `activity` — record sync runs and entries to SQLite
//! - `stats` — compute dashboard aggregations
//! - `merge` — remote + cached merge algorithm

pub mod auth_service;
pub mod discovery;
pub mod merge;
pub mod stats;
pub mod sync_engine;

use std::path::PathBuf;
use std::sync::Arc;

use git_vacuum_core::{
    Database, GithubApi, GitOps, KeyringStore, PartialSyncSummary, RepoEntry, SyncOptions,
    SyncSummary, UserInfo,
};
use tokio::sync::mpsc;
use tokio::sync::watch;

pub struct Services {
    pub github: Arc<dyn GithubApi>,
    pub git: Arc<dyn GitOps>,
    pub db: Arc<dyn Database>,
    pub keyring: Arc<dyn KeyringStore>,
}

impl Services {
    pub fn new(
        github: Arc<dyn GithubApi>,
        git: Arc<dyn GitOps>,
        db: Arc<dyn Database>,
        keyring: Arc<dyn KeyringStore>,
    ) -> Self {
        Self { github, git, db, keyring }
    }
}

pub struct SyncRequest {
    pub repos: Vec<RepoEntry>,
    pub base_path: PathBuf,
    pub concurrency: usize,
    pub options: SyncOptions,
}

pub async fn run_sync(
    services: Arc<Services>,
    request: SyncRequest,
    progress_tx: mpsc::UnboundedSender<git_vacuum_core::AppEvent>,
    app_tx: mpsc::UnboundedSender<git_vacuum_core::AppEvent>,
    cancel_rx: watch::Receiver<bool>,
) -> SyncSummary {
    sync_engine::run(services, request, progress_tx, app_tx, cancel_rx).await
}

pub async fn run_discovery(
    services: Arc<Services>,
    source: git_vacuum_core::RepoSource,
) -> Result<Vec<RepoEntry>, git_vacuum_core::DiscoveryError> {
    discovery::discover(services, source).await
}

pub async fn authenticate_pat(
    services: Arc<Services>,
    token: &str,
) -> Result<UserInfo, git_vacuum_core::AuthError> {
    auth_service::authenticate_pat(services, token).await
}

pub async fn load_stored_credentials(
    services: Arc<Services>,
) -> Result<Option<UserInfo>, git_vacuum_core::AuthError> {
    auth_service::load_stored_credentials(services).await
}

pub async fn logout(services: Arc<Services>) -> Result<(), git_vacuum_core::KeyringError> {
    auth_service::logout(services).await
}

pub async fn start_oauth_device_flow(
    services: Arc<Services>,
    client_id: &str,
) -> Result<git_vacuum_core::DeviceFlowInit, git_vacuum_core::AuthError> {
    auth_service::start_oauth_device_flow(services, client_id).await
}

pub async fn poll_oauth_device_flow(
    services: Arc<Services>,
    client_id: &str,
    device_code: String,
) -> Result<git_vacuum_core::DeviceFlowPoll, git_vacuum_core::AuthError> {
    auth_service::poll_oauth_device_flow(services, client_id, device_code).await
}

pub async fn complete_oauth_with_token(
    services: Arc<Services>,
    token: String,
) -> Result<UserInfo, git_vacuum_core::AuthError> {
    auth_service::complete_oauth_with_token(services, token).await
}

pub fn oauth_poll_interval() -> std::time::Duration {
    auth_service::default_poll_interval()
}

pub async fn compute_stats(
    services: Arc<Services>,
) -> Result<git_vacuum_core::DashboardStats, git_vacuum_core::DbError> {
    stats::compute(services).await
}

pub fn summarize_partial(
    completed: usize,
    failed: usize,
    cancelled: usize,
    pending_dropped: usize,
    bytes_transferred: u64,
) -> PartialSyncSummary {
    PartialSyncSummary { completed, failed, cancelled, pending_dropped, bytes_transferred }
}
