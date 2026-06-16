use std::sync::Arc;

use git_vacuum_core::{AppEvent, RepoSource, RemoteRepo, RepoEntry};
use git_vacuum_core::traits::{Database, GithubApi};
use git_vacuum_core::types::repo::CloneStatus;
use tokio::sync::mpsc;

pub async fn discover_repos(
    source: RepoSource,
    github: Arc<dyn GithubApi>,
    db: Arc<dyn Database>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
    cancel_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<Vec<RepoEntry>, String> {
    let remote_repos = match &source {
        RepoSource::MyRepos => github.list_my_repos().await?,
        RepoSource::OrgRepos(org) => github.list_org_repos(org).await?,
        RepoSource::Starred => github.list_starred_repos().await?,
        RepoSource::AllAccessible => github.list_all_accessible_repos().await?,
    };

    let _ = app_tx.send(AppEvent::DiscoveryProgress {
        repos_found: remote_repos.len(),
        estimated_total: Some(remote_repos.len()),
    });

    merge_and_cache(remote_repos, &source, db, app_tx).await
}

async fn merge_and_cache(
    remote_repos: Vec<RemoteRepo>,
    source: &RepoSource,
    db: Arc<dyn Database>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
) -> Result<Vec<RepoEntry>, String> {
    let cached = db.get_all_repos().await?;
    let mut cached_by_id: std::collections::HashMap<i64, git_vacuum_core::traits::database::RepoRow> = cached
        .into_iter()
        .map(|r| (r.github_id, r))
        .collect();

    let mut entries = Vec::new();

    for remote in &remote_repos {
        let (clone_status, local_path, behind_count, last_synced_at, last_error, selected) =
            if let Some(cached) = cached_by_id.get(&remote.github_id) {
                let status = match cached.clone_status.as_str() {
                    "cloned" => CloneStatus::Cloned,
                    "stale" => CloneStatus::Stale,
                    "error" => CloneStatus::Error,
                    _ => CloneStatus::NotCloned,
                };
                (status, cached.local_path.clone(), cached.behind_count,
                 cached.last_synced_at.clone(), cached.last_error.clone(), cached.selected)
            } else {
                (CloneStatus::NotCloned, None, 0, None, None, true)
            };

        entries.push(RepoEntry {
            github_id: remote.github_id,
            owner_login: remote.owner_login.clone(),
            name: remote.name.clone(),
            full_name: remote.full_name.clone(),
            description: remote.description.clone(),
            language: remote.language.clone(),
            default_branch: remote.default_branch.clone(),
            visibility: remote.visibility.clone(),
            is_fork: remote.is_fork,
            is_archived: remote.is_archived,
            is_template: remote.is_template,
            size_kb: remote.size_kb,
            stars: remote.stars,
            open_issues: remote.open_issues,
            license_spdx: remote.license_spdx.clone(),
            topics: remote.topics.clone(),
            clone_url_ssh: remote.clone_url_ssh.clone(),
            clone_url_https: remote.clone_url_https.clone(),
            homepage_url: remote.homepage_url.clone(),
            pushed_at: remote.pushed_at,
            clone_status,
            local_path,
            local_size_kb: None,
            behind_count,
            ahead_count: 0,
            last_synced_at: last_synced_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&chrono::Utc))),
            last_error,
            last_error_at: None,
            selected,
            deleted_on_remote: false,
            discovered_at: Some(chrono::Utc::now()),
        });
    }

    let _ = app_tx.send(AppEvent::ReposDiscovered {
        repos: entries.clone(),
        source: source.clone(),
    });

    Ok(entries)
}
