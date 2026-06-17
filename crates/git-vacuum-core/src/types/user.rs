use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Pat,
    OAuthDeviceFlow,
    GhCli,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub github_user_id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub scopes: Vec<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
}
