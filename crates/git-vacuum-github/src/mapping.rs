use chrono::{DateTime, Utc};
use git_vacuum_core::{RemoteRepo, RepoVisibility, UserInfo};

pub fn map_repo(r: octocrab::models::Repository) -> RemoteRepo {
    let visibility = match r.visibility.as_deref() {
        Some("private") => RepoVisibility::Private,
        Some("internal") => RepoVisibility::Internal,
        _ => RepoVisibility::Public,
    };
    let owner_login = r.owner.as_ref().map(|o| o.login.clone()).unwrap_or_default();
    let owner_is_org = r.owner.as_ref().map(|o| o.r#type == "Organization").unwrap_or(false);

    RemoteRepo {
        github_id: r.id.0 as i64,
        owner_login,
        name: r.name,
        full_name: r.full_name.unwrap_or_default(),
        description: r.description,
        language: r.language.as_ref().and_then(|v| {
            let obj = v.as_object()?;
            obj.keys().next().cloned()
        }),
        default_branch: r.default_branch.unwrap_or_else(|| "main".into()),
        visibility,
        is_fork: r.fork.unwrap_or(false),
        is_archived: r.archived.unwrap_or(false),
        is_template: r.is_template.unwrap_or(false),
        size_kb: r.size.map(|s| s as i64),
        stars: r.stargazers_count.map(|c| c as i32).unwrap_or(0),
        open_issues: r.open_issues_count.map(|c| c as i32).unwrap_or(0),
        license_spdx: r.license.and_then(|l| if l.spdx_id.is_empty() { None } else { Some(l.spdx_id) }),
        topics: r.topics.unwrap_or_default(),
        clone_url_ssh: r.ssh_url,
        clone_url_https: r.clone_url.map(|u| u.to_string()).unwrap_or_default(),
        homepage_url: r.homepage,
        pushed_at: r.pushed_at,
        created_at: r.created_at.unwrap_or_else(Utc::now),
        updated_at: r.updated_at.unwrap_or_else(Utc::now),
        owner_is_org,
    }
}

pub fn parse_user(
    user: octocrab::models::Author,
    scopes: Vec<String>,
    token_expires_at: Option<DateTime<Utc>>,
) -> UserInfo {
    UserInfo {
        github_user_id: user.id.0 as i64,
        login: user.login,
        name: None,
        email: user.email,
        avatar_url: Some(user.avatar_url.to_string()),
        scopes,
        token_expires_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_from_json() {
        let json = r#"{
            "login": "alice",
            "id": 42,
            "node_id": "abc",
            "avatar_url": "https://example.com/a.png",
            "gravatar_id": "",
            "url": "https://api.github.com/users/alice",
            "html_url": "https://github.com/alice",
            "followers_url": "https://api.github.com/users/alice/followers",
            "following_url": "https://api.github.com/users/alice/following",
            "gists_url": "https://api.github.com/users/alice/gists",
            "starred_url": "https://api.github.com/users/alice/starred",
            "subscriptions_url": "https://api.github.com/users/alice/subscriptions",
            "organizations_url": "https://api.github.com/users/alice/orgs",
            "repos_url": "https://api.github.com/users/alice/repos",
            "events_url": "https://api.github.com/users/alice/events",
            "received_events_url": "https://api.github.com/users/alice/received_events",
            "type": "User",
            "site_admin": false,
            "email": "alice@example.com"
        }"#;
        let user: octocrab::models::Author = serde_json::from_str(json).unwrap();
        let info = parse_user(user, vec!["repo".into()], None);
        assert_eq!(info.github_user_id, 42);
        assert_eq!(info.login, "alice");
        assert_eq!(info.email.as_deref(), Some("alice@example.com"));
        assert_eq!(info.scopes, vec!["repo".to_string()]);
    }
}
