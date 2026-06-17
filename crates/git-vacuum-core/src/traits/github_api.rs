use std::time::Duration;

use async_trait::async_trait;

use crate::types::org::OrgInfo;
use crate::types::repo::{RemoteRepo, RepoSource};
use crate::types::user::UserInfo;

#[derive(Debug, Clone)]
pub struct DeviceFlowInit {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: Duration,
    pub interval: Duration,
}

#[derive(Debug, Clone)]
pub enum DeviceFlowPoll {
    Pending,
    Success { access_token: String, scopes: Vec<String> },
    SlowDown { new_interval: Duration },
    Expired,
    AccessDenied,
}

pub struct PagedStream<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> PagedStream<T> {
    pub fn empty() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

#[async_trait]
pub trait GithubApi: Send + Sync {
    fn set_token(&self, token: &str);
    fn clear_token(&self);

    async fn validate_token(&self) -> Result<UserInfo, crate::error::AuthError>;
    async fn get_authenticated_user(&self) -> Result<UserInfo, crate::error::AuthError>;

    async fn list_my_repos(&self) -> Result<Vec<RemoteRepo>, crate::error::DiscoveryError>;
    async fn list_org_repos(&self, org: &str) -> Result<Vec<RemoteRepo>, crate::error::DiscoveryError>;
    async fn list_starred_repos(&self) -> Result<Vec<RemoteRepo>, crate::error::DiscoveryError>;
    async fn list_all_accessible_repos(&self) -> Result<Vec<RemoteRepo>, crate::error::DiscoveryError>;

    async fn list_my_orgs(&self) -> Result<Vec<OrgInfo>, crate::error::DiscoveryError>;

    async fn device_flow_init(
        &self,
        client_id: &str,
        scopes: &[&str],
    ) -> Result<DeviceFlowInit, crate::error::AuthError>;
    async fn device_flow_poll(
        &self,
        client_id: &str,
        device_code: &str,
    ) -> Result<DeviceFlowPoll, crate::error::AuthError>;

    async fn get_rate_limit(&self) -> Result<RateLimitStatus, crate::error::DiscoveryError>;
}

#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: chrono::DateTime<chrono::Utc>,
    pub resource: String,
}

impl RateLimitStatus {
    pub fn display(&self) -> String {
        format!(
            "{}/{} (resets {})",
            self.remaining,
            self.limit,
            self.reset_at.format("%H:%M")
        )
    }
}

pub async fn list_for_source(
    api: &dyn GithubApi,
    source: &RepoSource,
) -> Result<Vec<RemoteRepo>, crate::error::DiscoveryError> {
    match source {
        RepoSource::MyRepos => api.list_my_repos().await,
        RepoSource::Org { login } => api.list_org_repos(login).await,
        RepoSource::Starred => api.list_starred_repos().await,
        RepoSource::All => api.list_all_accessible_repos().await,
    }
}
