use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub github_user_id: i64,
    pub login: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub scopes: Vec<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    PersonalAccessToken(String),
    OAuthDeviceFlow,
    GhCliToken,
}
