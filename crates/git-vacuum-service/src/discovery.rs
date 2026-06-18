use std::sync::Arc;

use git_vacuum_core::{list_for_source, DiscoveryError, RemoteRepo, RepoEntry, RepoSource};

use crate::merge::{merge_remote_into, should_prune_from_scope};
use crate::Services;

/// Fetch remote repos from GitHub, merge with cached entries, persist.
/// Returns the new merged list (suitable for the UI's Explorer).
pub async fn discover(
    services: Arc<Services>,
    source: RepoSource,
) -> Result<Vec<RepoEntry>, DiscoveryError> {
    // 1. Validate token first (fail fast)
    services
        .github
        .get_authenticated_user()
        .await
        .map_err(|e| DiscoveryError::Auth(format!("{e}")))?;

    // 2. Fetch remote repos
    let remote = list_for_source(services.github.as_ref(), &source).await?;

    // 3. Load cached entries
    let cached = services
        .db
        .get_all_repos()
        .map_err(|e| DiscoveryError::Internal(e.to_string()))?;
    let cached_by_id: std::collections::HashMap<i64, _> =
        cached.iter().map(|r| (r.github_id, r.clone())).collect();

    // 4. Build merged entries
    let mut merged: Vec<RepoEntry> = Vec::with_capacity(remote.len());
    for r in remote {
        let existing_repo_row = cached_by_id.get(&r.github_id);
        // Try to find a matching RepoEntry for state preservation
        let existing_entry: Option<RepoEntry> = existing_repo_row.map(row_to_entry);
        let new = existing_repo_row.map(|r| r.selected).unwrap_or(true);
        merged.push(merge_remote_into(r, existing_entry.as_ref(), new));
    }

    // 5. Mark cached-but-not-in-remote as deleted_on_remote (if in scope)
    let remote_ids: std::collections::HashSet<i64> = merged.iter().map(|e| e.github_id).collect();
    for cached_entry in &cached {
        if !remote_ids.contains(&cached_entry.github_id)
            && should_prune_from_scope(&row_to_entry(cached_entry), &source)
        {
            let _ = services
                .db
                .mark_repo_deleted_on_remote(cached_entry.github_id);
        }
    }

    // 6. Persist merged list
    let rows: Vec<_> = merged.iter().map(entry_to_row).collect();
    services
        .db
        .upsert_repos(&rows)
        .map_err(|e| DiscoveryError::Internal(e.to_string()))?;

    Ok(merged)
}

/// Check whether a repo is within the scope of a discovery source.
/// Public so `merge::should_prune_from_scope` can use it.
pub fn is_in_scope(repo: &RepoEntry, source: &RepoSource) -> bool {
    match source {
        RepoSource::MyRepos => {
            // We approximate "my repos" as "not in any org" — we don't track owner_is_org here
            // since RepoEntry doesn't carry it. The org filter for "all" will catch the rest.
            true
        }
        RepoSource::Org { login } => repo.owner_login.eq_ignore_ascii_case(login),
        RepoSource::Starred => true,
        RepoSource::All => true,
    }
}

#[allow(dead_code)]
pub async fn fetch_remote_repos(
    services: Arc<Services>,
    source: &RepoSource,
) -> Result<Vec<RemoteRepo>, DiscoveryError> {
    list_for_source(services.github.as_ref(), source).await
}

fn row_to_entry(row: &git_vacuum_core::RepoRow) -> RepoEntry {
    let topics: Vec<String> = row
        .topics_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    RepoEntry {
        github_id: row.github_id,
        owner_login: row.owner.clone(),
        name: row.name.clone(),
        full_name: row.full_name.clone(),
        description: row.description.clone(),
        language: row.language.clone(),
        default_branch: row.default_branch.clone(),
        visibility: match row.visibility.as_str() {
            "private" => git_vacuum_core::RepoVisibility::Private,
            "internal" => git_vacuum_core::RepoVisibility::Internal,
            _ => git_vacuum_core::RepoVisibility::Public,
        },
        is_fork: row.is_fork,
        is_archived: row.is_archived,
        size_kb: row.size_kb,
        stars: row.stars,
        pushed_at: row.pushed_at,
        updated_at: row.updated_at,
        topics,
        clone_url_https: row.clone_url_https.clone(),
        clone_url_ssh: row.clone_url_ssh.clone(),
        clone_status: row.clone_status,
        local_path: row.local_path.clone(),
        local_size_kb: row.local_size_kb,
        last_synced_at: row.last_synced_at,
        last_error: row.last_error.clone(),
        behind_count: row.behind_count,
        selected: row.selected,
        deleted_on_remote: row.deleted_on_remote,
        discovered_at: row.discovered_at,
    }
}

fn entry_to_row(e: &RepoEntry) -> git_vacuum_core::RepoRow {
    let visibility = match e.visibility {
        git_vacuum_core::RepoVisibility::Public => "public",
        git_vacuum_core::RepoVisibility::Private => "private",
        git_vacuum_core::RepoVisibility::Internal => "internal",
    };
    let topics_json = serde_json::to_string(&e.topics).ok();
    git_vacuum_core::RepoRow {
        id: 0,
        github_id: e.github_id,
        owner: e.owner_login.clone(),
        name: e.name.clone(),
        full_name: e.full_name.clone(),
        description: e.description.clone(),
        language: e.language.clone(),
        stars: e.stars,
        default_branch: e.default_branch.clone(),
        visibility: visibility.to_string(),
        is_fork: e.is_fork,
        is_archived: e.is_archived,
        clone_url_ssh: e.clone_url_ssh.clone(),
        clone_url_https: e.clone_url_https.clone(),
        size_kb: e.size_kb,
        pushed_at: e.pushed_at,
        created_at: e.discovered_at,
        updated_at: e.updated_at,
        clone_status: e.clone_status,
        local_path: e.local_path.clone(),
        local_size_kb: e.local_size_kb,
        last_synced_at: e.last_synced_at,
        last_error: e.last_error.clone(),
        behind_count: e.behind_count,
        selected: e.selected,
        discovered_at: e.discovered_at,
        deleted_on_remote: e.deleted_on_remote,
        topics_json,
    }
}
