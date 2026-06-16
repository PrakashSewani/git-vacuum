use std::sync::Mutex;
use octocrab::Octocrab;
use git_vacuum_core::traits::github_api::GithubApi;

pub struct OctocrabGithubApi {
    client: Mutex<Octocrab>,
}

impl OctocrabGithubApi {
    pub fn new(base_url: Option<String>, _user_agent: String) -> Result<Self, String> {
        let mut builder = Octocrab::builder();
        if let Some(url) = base_url {
            builder = builder.base_uri(url).map_err(|e| format!("Invalid base URL: {}", e))?;
        }
        let client = builder.build().map_err(|e| format!("Failed to build client: {}", e))?;
        Ok(Self { client: Mutex::new(client) })
    }

    fn map_repo(repo: octocrab::models::Repository) -> git_vacuum_core::RemoteRepo {
        use git_vacuum_core::{RemoteRepo, RepoVisibility};
        let owner_type = repo.owner.as_ref().map(|o| o.r#type == "Organization").unwrap_or(false);
        RemoteRepo {
            github_id: repo.id.into_inner() as i64,
            owner_login: repo.owner.as_ref().map(|o| o.login.clone()).unwrap_or_default(),
            name: repo.name,
            full_name: repo.full_name.unwrap_or_default(),
            description: repo.description,
            language: repo.language.map(|l| l.to_string()),
            default_branch: repo.default_branch.unwrap_or_else(|| "main".to_string()),
            visibility: match repo.visibility.as_deref() {
                Some("private") => RepoVisibility::Private,
                Some("internal") => RepoVisibility::Internal,
                _ => RepoVisibility::Public,
            },
            is_fork: repo.fork.unwrap_or(false),
            is_archived: repo.archived.unwrap_or(false),
            is_template: repo.is_template.unwrap_or(false),
            size_kb: repo.size.map(|s| s as i64),
            stars: repo.stargazers_count.unwrap_or(0) as i32,
            open_issues: repo.open_issues_count.unwrap_or(0) as i32,
            license_spdx: repo.license.and_then(|l| Some(l.spdx_id)),
            topics: repo.topics.unwrap_or_default(),
            clone_url_ssh: repo.ssh_url,
            clone_url_https: repo.clone_url.as_ref().map(|u| u.to_string()).unwrap_or_default(),
            homepage_url: repo.homepage.as_ref().map(|u| u.to_string()),
            pushed_at: repo.pushed_at,
            created_at: repo.created_at,
            updated_at: repo.updated_at,
            owner_is_org: owner_type,
        }
    }
}

#[async_trait::async_trait]
impl GithubApi for OctocrabGithubApi {
    async fn set_token(&self, token: &str) {
        let new_client = Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .unwrap_or_else(|e| {
                log::error!("Failed to build authenticated Octocrab client: {}", e);
                panic!("Cannot create authenticated client: {}", e);
            });
        let mut guard = self.client.lock().unwrap();
        *guard = new_client;
    }

    async fn validate_token(&self) -> Result<git_vacuum_core::UserInfo, String> {
        let client = self.client.lock().unwrap().clone();
        let user: octocrab::models::Author = client
            .get("/user", None::<&()>)
            .await
            .map_err(|e| format!("Token validation failed: {}", e))?;

        Ok(git_vacuum_core::UserInfo {
            github_user_id: user.id.into_inner() as i64,
            login: user.login,
            display_name: None,
            email: user.email,
            avatar_url: Some(user.avatar_url.to_string()),
            scopes: vec![],
            token_expires_at: None,
        })
    }

    async fn get_authenticated_user(&self) -> Result<git_vacuum_core::UserInfo, String> {
        self.validate_token().await
    }

    async fn device_flow_init(
        &self,
        client_id: &str,
        scopes: &[&str],
    ) -> Result<git_vacuum_core::DeviceFlowInit, String> {
        #[derive(serde::Serialize)]
        struct DeviceCodeRequest { client_id: String, scope: String }
        #[derive(serde::Deserialize)]
        struct DeviceCodeResponse {
            device_code: String, user_code: String,
            verification_uri: String, expires_in: u64, interval: u64,
        }
        let req = DeviceCodeRequest { client_id: client_id.to_string(), scope: scopes.join(" ") };
        let client = self.client.lock().unwrap().clone();
        let resp: DeviceCodeResponse = client
            .post("https://github.com/login/device/code", Some(&req))
            .await
            .map_err(|e| format!("Device flow init failed: {}", e))?;
        Ok(git_vacuum_core::DeviceFlowInit {
            device_code: resp.device_code, user_code: resp.user_code,
            verification_uri: resp.verification_uri,
            expires_in: std::time::Duration::from_secs(resp.expires_in),
            interval: std::time::Duration::from_secs(resp.interval),
        })
    }

    async fn device_flow_poll(
        &self, client_id: &str, device_code: &str,
    ) -> Result<git_vacuum_core::DeviceFlowPoll, String> {
        #[derive(serde::Serialize)]
        struct PollRequest { client_id: String, device_code: String, grant_type: String }
        #[derive(serde::Deserialize)]
        struct PollResponse {
            #[serde(default)] access_token: Option<String>,
            #[serde(default)] error: Option<String>,
            #[serde(default)] interval: Option<u64>,
        }
        let req = PollRequest {
            client_id: client_id.to_string(), device_code: device_code.to_string(),
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
        };
        let client = self.client.lock().unwrap().clone();
        let resp: PollResponse = client
            .post("https://github.com/login/oauth/access_token", Some(&req))
            .await
            .map_err(|e| format!("Device poll failed: {}", e))?;
        if let Some(token) = resp.access_token {
            Ok(git_vacuum_core::DeviceFlowPoll::Success { access_token: token, scopes: vec![] })
        } else {
            match resp.error.as_deref() {
                Some("authorization_pending") => Ok(git_vacuum_core::DeviceFlowPoll::Pending),
                Some("slow_down") => Ok(git_vacuum_core::DeviceFlowPoll::SlowDown {
                    new_interval: std::time::Duration::from_secs(resp.interval.unwrap_or(10)),
                }),
                Some("expired_token") => Ok(git_vacuum_core::DeviceFlowPoll::Expired),
                Some("access_denied") => Ok(git_vacuum_core::DeviceFlowPoll::AccessDenied),
                _ => Err("Unknown OAuth error".to_string()),
            }
        }
    }

    async fn list_my_orgs(&self) -> Result<Vec<git_vacuum_core::OrgInfo>, String> {
        let client = self.client.lock().unwrap().clone();
        let orgs: Vec<octocrab::models::orgs::Organization> = client
            .get("/user/orgs", None::<&()>)
            .await
            .map_err(|e| format!("List orgs failed: {}", e))?;
        Ok(orgs.into_iter().map(|o| git_vacuum_core::OrgInfo {
            github_org_id: o.id.into_inner() as i64,
            login: o.login, display_name: o.name,
            description: o.description, avatar_url: Some(o.avatar_url.to_string()),
            repos_count: 0, discovered_at: chrono::Utc::now(),
        }).collect())
    }

    async fn list_my_repos(&self) -> Result<Vec<git_vacuum_core::RemoteRepo>, String> {
        let client = self.client.lock().unwrap().clone();
        let repos: Vec<octocrab::models::Repository> = client
            .get("/user/repos?affiliation=owner,collaborator,organization_member&sort=updated&per_page=100", None::<&()>)
            .await
            .map_err(|e| format!("List repos failed: {}", e))?;
        Ok(repos.into_iter().map(Self::map_repo).collect())
    }

    async fn list_org_repos(&self, org: &str) -> Result<Vec<git_vacuum_core::RemoteRepo>, String> {
        let client = self.client.lock().unwrap().clone();
        let repos: Vec<octocrab::models::Repository> = client
            .get(&format!("/orgs/{}/repos?type=all&sort=updated&per_page=100", org), None::<&()>)
            .await
            .map_err(|e| format!("List org repos failed: {}", e))?;
        Ok(repos.into_iter().map(Self::map_repo).collect())
    }

    async fn list_starred_repos(&self) -> Result<Vec<git_vacuum_core::RemoteRepo>, String> {
        let client = self.client.lock().unwrap().clone();
        let repos: Vec<octocrab::models::Repository> = client
            .get("/user/starred?sort=created&per_page=100", None::<&()>)
            .await
            .map_err(|e| format!("List starred failed: {}", e))?;
        Ok(repos.into_iter().map(Self::map_repo).collect())
    }

    async fn list_all_accessible_repos(&self) -> Result<Vec<git_vacuum_core::RemoteRepo>, String> {
        let my_repos = self.list_my_repos().await?;
        let orgs = self.list_my_orgs().await?;
        let mut all_repos = my_repos;
        for org in &orgs {
            if let Ok(org_repos) = self.list_org_repos(&org.login).await {
                for repo in org_repos {
                    if !all_repos.iter().any(|r| r.github_id == repo.github_id) {
                        all_repos.push(repo);
                    }
                }
            }
        }
        Ok(all_repos)
    }
}
