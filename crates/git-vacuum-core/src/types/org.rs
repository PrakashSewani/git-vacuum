use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgInfo {
    pub github_org_id: i64,
    pub login: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub avatar_url: Option<String>,
    pub repos_count: i32,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMembership {
    pub org_id: i64,
    pub role: String,
    pub joined_at: Option<DateTime<Utc>>,
}
