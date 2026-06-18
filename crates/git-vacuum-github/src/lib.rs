pub mod error;
pub mod mapping;

use async_trait::async_trait;
use chrono::Utc;
use git_vacuum_core::{
    AuthError, DeviceFlowInit, DeviceFlowPoll, DiscoveryError, GithubApi, OrgInfo, RateLimitStatus,
    RemoteRepo, UserInfo,
};
use octocrab::Octocrab;
use parking_lot::Mutex;

use crate::error::{map_auth_error, map_discovery_error};
use crate::mapping::parse_user;

pub struct OctocrabGithubApi {
    base_url: String,
    user_agent: String,
    token: Mutex<Option<String>>,
    client: Mutex<Option<Octocrab>>,
}

impl OctocrabGithubApi {
    pub fn new(base_url: impl Into<String>, user_agent: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            user_agent: user_agent.into(),
            token: Mutex::new(None),
            client: Mutex::new(None),
        }
    }

    /// The OAuth device-flow and access-token endpoints live on the main
    /// GitHub web host (https://github.com), NOT on the REST API host
    /// (https://api.github.com). When the configured base URL is the API
    /// host (the default), we transparently swap it for the OAuth calls.
    /// For GitHub Enterprise, the user supplies a custom base_url like
    /// `https://github.acme.com` which serves both — in that case this
    /// function returns it unchanged.
    fn oauth_base_url(&self) -> String {
        if self.base_url.contains("api.github.com") {
            "https://github.com".to_string()
        } else {
            self.base_url.clone()
        }
    }

    fn unauth_client(&self) -> Result<Octocrab, AuthError> {
        if let Some(c) = self.client.lock().clone() {
            return Ok(c);
        }
        let crab = Octocrab::builder()
            .base_uri(self.base_url.as_str())
            .map_err(|e| AuthError::Internal(format!("invalid base uri: {e}")))?
            .add_header(
                http::HeaderName::from_static("user-agent"),
                self.user_agent.clone(),
            )
            .build()
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        *self.client.lock() = Some(crab.clone());
        Ok(crab)
    }

    fn auth_client(&self) -> Result<Octocrab, AuthError> {
        let token = self.token.lock().clone();
        let token = token.ok_or(AuthError::InvalidToken)?;
        Octocrab::builder()
            .base_uri(self.base_url.as_str())
            .map_err(|e| AuthError::Internal(format!("invalid base uri: {e}")))?
            .add_header(
                http::HeaderName::from_static("user-agent"),
                self.user_agent.clone(),
            )
            .personal_token(token)
            .build()
            .map_err(|e| AuthError::Internal(e.to_string()))
    }
}

fn auth_to_discovery(e: AuthError) -> DiscoveryError {
    match e {
        AuthError::InvalidToken => DiscoveryError::Auth("invalid token".into()),
        AuthError::Network(s) => DiscoveryError::Network(s),
        other => DiscoveryError::Internal(other.to_string()),
    }
}

#[async_trait]
impl GithubApi for OctocrabGithubApi {
    fn set_token(&self, token: &str) {
        *self.token.lock() = Some(token.to_string());
        *self.client.lock() = None;
    }

    fn clear_token(&self) {
        *self.token.lock() = None;
        *self.client.lock() = None;
    }

    async fn validate_token(&self) -> Result<UserInfo, AuthError> {
        let crab = self.auth_client()?;
        let user = crab.current().user().await.map_err(map_auth_error)?;
        Ok(parse_user(user, vec![], None))
    }

    async fn get_authenticated_user(&self) -> Result<UserInfo, AuthError> {
        self.validate_token().await
    }

    async fn list_my_repos(&self) -> Result<Vec<RemoteRepo>, DiscoveryError> {
        let crab = self.auth_client().map_err(auth_to_discovery)?;
        let url = format!(
            "{}/user/repos?per_page=100&affiliation=owner,collaborator,organization_member",
            self.base_url
        );
        let page: Vec<octocrab::models::Repository> = crab
            .get(url, None::<&()>)
            .await
            .map_err(map_discovery_error)?;
        Ok(page.into_iter().map(mapping::map_repo).collect())
    }

    async fn list_org_repos(&self, org: &str) -> Result<Vec<RemoteRepo>, DiscoveryError> {
        let crab = self.auth_client().map_err(auth_to_discovery)?;
        let url = format!(
            "{}/orgs/{}/repos?per_page=100&type=all&sort=updated",
            self.base_url, org
        );
        let page: Vec<octocrab::models::Repository> = crab
            .get(url, None::<&()>)
            .await
            .map_err(map_discovery_error)?;
        Ok(page.into_iter().map(mapping::map_repo).collect())
    }

    async fn list_starred_repos(&self) -> Result<Vec<RemoteRepo>, DiscoveryError> {
        let crab = self.auth_client().map_err(auth_to_discovery)?;
        let url = format!("{}/user/starred?per_page=100&sort=created", self.base_url);
        let page: Vec<octocrab::models::Repository> = crab
            .get(url, None::<&()>)
            .await
            .map_err(map_discovery_error)?;
        Ok(page.into_iter().map(mapping::map_repo).collect())
    }

    async fn list_all_accessible_repos(&self) -> Result<Vec<RemoteRepo>, DiscoveryError> {
        let mut all = self.list_my_repos().await?;
        let orgs = self.list_my_orgs().await?;
        for org in orgs {
            let mut org_repos = self.list_org_repos(&org.login).await?;
            all.append(&mut org_repos);
        }
        all.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        all.dedup_by_key(|r| r.github_id);
        Ok(all)
    }

    async fn list_my_orgs(&self) -> Result<Vec<OrgInfo>, DiscoveryError> {
        let crab = self.auth_client().map_err(auth_to_discovery)?;
        let url = format!("{}/user/orgs?per_page=100", self.base_url);
        let page: Vec<octocrab::models::orgs::Organization> = crab
            .get(url, None::<&()>)
            .await
            .map_err(map_discovery_error)?;
        Ok(page
            .into_iter()
            .map(|o| OrgInfo {
                github_org_id: o.id.0 as i64,
                login: o.login,
                display_name: o.name,
                description: o.description,
                avatar_url: Some(o.avatar_url.to_string()),
                repos_count: o.public_repos.map(|c| c as i32).unwrap_or(0),
            })
            .collect())
    }

    async fn device_flow_init(
        &self,
        client_id: &str,
        scopes: &[&str],
    ) -> Result<DeviceFlowInit, AuthError> {
        let client = reqwest::Client::new();
        let scope = scopes.join(" ");
        let res = client
            .post(format!("{}/login/device/code", self.oauth_base_url()))
            .header("Accept", "application/json")
            .header("User-Agent", &self.user_agent)
            .form(&[("client_id", client_id), ("scope", &scope)])
            .send()
            .await
            .map_err(|e| AuthError::Network(e.to_string()))?;

        // Check HTTP status — GitHub returns 4xx for OAuth errors with the
        // body containing { "error": "...", "error_description": "..." }
        let status = res.status();
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| AuthError::Internal(format!("json: {e}")))?;

        if let Some(err) = body["error"].as_str() {
            let desc = body["error_description"]
                .as_str()
                .unwrap_or("(no description)");
            return Err(match err {
                "incorrect_client_credentials" | "invalid_client" => {
                    AuthError::Internal(format!(
                        "OAuth client_id invalid: {desc}. Register at https://github.com/settings/applications/new"
                    ))
                }
                "unauthorized_client" => AuthError::Internal(format!(
                    "OAuth client_id not authorized for device flow: {desc}"
                )),
                "unsupported_grant_type" | "invalid_request" => {
                    AuthError::Internal(format!("OAuth init rejected: {desc}"))
                }
                _ => AuthError::Internal(format!("OAuth init error ({err}): {desc}")),
            });
        }
        if !status.is_success() {
            return Err(AuthError::Internal(format!(
                "OAuth init HTTP {}: {}",
                status, body
            )));
        }

        Ok(DeviceFlowInit {
            device_code: body["device_code"].as_str().unwrap_or_default().to_string(),
            user_code: body["user_code"].as_str().unwrap_or_default().to_string(),
            verification_uri: body["verification_uri"]
                .as_str()
                .unwrap_or("https://github.com/login/device")
                .to_string(),
            expires_in: std::time::Duration::from_secs(body["expires_in"].as_u64().unwrap_or(900)),
            interval: std::time::Duration::from_secs(body["interval"].as_u64().unwrap_or(5)),
        })
    }

    async fn device_flow_poll(
        &self,
        client_id: &str,
        device_code: &str,
    ) -> Result<DeviceFlowPoll, AuthError> {
        let client = reqwest::Client::new();
        let res = client
            .post(format!(
                "{}/login/oauth/access_token",
                self.oauth_base_url()
            ))
            .header("Accept", "application/json")
            .header("User-Agent", &self.user_agent)
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| AuthError::Network(e.to_string()))?;

        let status = res.status();
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| AuthError::Internal(format!("json: {e}")))?;

        // GitHub can return errors as either { "error": "...", ... } or
        // 4xx with { "message": "...", "documentation_url": "..." }.
        // Handle both.
        let err_str = body["error"].as_str();
        let desc_str = body["error_description"]
            .as_str()
            .or_else(|| body["message"].as_str())
            .unwrap_or("");
        if let Some(err) = err_str {
            log::debug!("OAuth poll error: {err} — {desc_str}");
        }

        match err_str {
            Some("authorization_pending") => Ok(DeviceFlowPoll::Pending),
            Some("slow_down") => {
                let new_interval = std::time::Duration::from_secs(body["interval"].as_u64().unwrap_or(10));
                Ok(DeviceFlowPoll::SlowDown { new_interval })
            }
            Some("expired_token") => Ok(DeviceFlowPoll::Expired),
            Some("access_denied") => Ok(DeviceFlowPoll::AccessDenied),
            Some("incorrect_client_credentials") | Some("invalid_client") => {
                Err(AuthError::Internal(format!(
                    "OAuth client_id invalid: {desc_str}. Register at https://github.com/settings/applications/new"
                )))
            }
            Some("unsupported_grant_type") | Some("invalid_grant") => {
                Err(AuthError::Internal(format!(
                    "OAuth grant rejected: {desc_str}"
                )))
            }
            Some(other) => Err(AuthError::Internal(format!(
                "oauth error: {other} — {desc_str} (HTTP {status})"
            ))),
            None => {
                if !status.is_success() {
                    return Err(AuthError::Internal(format!(
                        "OAuth poll HTTP {status}: {}",
                        if desc_str.is_empty() { body.to_string() } else { desc_str.to_string() }
                    )));
                }
                let token = body["access_token"].as_str().ok_or_else(|| {
                    AuthError::Internal(format!(
                        "OAuth poll: success response missing access_token. Body: {}",
                        body
                    ))
                })?.to_string();
                let scope_str = body["scope"].as_str().unwrap_or("");
                let scopes: Vec<String> = scope_str.split_whitespace().map(|s| s.to_string()).collect();
                Ok(DeviceFlowPoll::Success { access_token: token, scopes })
            }
        }
    }

    async fn get_rate_limit(&self) -> Result<RateLimitStatus, DiscoveryError> {
        let crab = self.unauth_client().map_err(auth_to_discovery)?;
        let body: serde_json::Value = crab
            .get(format!("{}/rate_limit", self.base_url), None::<&()>)
            .await
            .map_err(map_discovery_error)?;
        let core = &body["resources"]["core"];
        let now = Utc::now().timestamp();
        let reset_epoch = core["reset"].as_i64().unwrap_or(now);
        Ok(RateLimitStatus {
            limit: core["limit"].as_u64().unwrap_or(5000) as u32,
            remaining: core["remaining"].as_u64().unwrap_or(5000) as u32,
            reset_at: Utc::now() + chrono::Duration::seconds((reset_epoch - now).max(0)),
            resource: "core".to_string(),
        })
    }
}
