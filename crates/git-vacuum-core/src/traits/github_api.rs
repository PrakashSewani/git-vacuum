use std::time::Duration;

use async_trait::async_trait;

use crate::types::org::OrgInfo;
use crate::types::repo::RemoteRepo;
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
    Success {
        access_token: String,
        scopes: Vec<String>,
    },
    SlowDown {
        new_interval: Duration,
    },
    Expired,
    AccessDenied,
}

#[async_trait]
pub trait GithubApi: Send + Sync {
    async fn set_token(&self, token: &str);
    async fn validate_token(&self) -> Result<UserInfo, String>;
    async fn get_authenticated_user(&self) -> Result<UserInfo, String>;

    async fn device_flow_init(
        &self,
        client_id: &str,
        scopes: &[&str],
    ) -> Result<DeviceFlowInit, String>;

    async fn device_flow_poll(
        &self,
        client_id: &str,
        device_code: &str,
    ) -> Result<DeviceFlowPoll, String>;

    async fn list_my_orgs(&self) -> Result<Vec<OrgInfo>, String>;
    async fn list_my_repos(&self) -> Result<Vec<RemoteRepo>, String>;
    async fn list_org_repos(&self, org: &str) -> Result<Vec<RemoteRepo>, String>;
    async fn list_starred_repos(&self) -> Result<Vec<RemoteRepo>, String>;
    async fn list_all_accessible_repos(&self) -> Result<Vec<RemoteRepo>, String>;
}
