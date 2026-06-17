use chrono::Utc;
use git_vacuum_core::{CloneStatus, RepoEntry, RepoSource, RemoteRepo};

use crate::discovery::is_in_scope;

/// Convert a fresh `RemoteRepo` from the GitHub API into a `RepoEntry` for the UI.
/// Existing `RepoEntry` data is preserved when the remote matches an existing row
/// (clone status, local path, selection, behind count, etc.).
pub fn merge_remote_into(
    remote: RemoteRepo,
    existing: Option<&RepoEntry>,
    new: bool,
) -> RepoEntry {
    let now = Utc::now();
    let topics_json = serde_json::to_string(&remote.topics).unwrap_or_else(|_| "[]".to_string());

    if let Some(existing) = existing {
        RepoEntry {
            github_id: remote.github_id,
            owner_login: remote.owner_login,
            name: remote.name,
            full_name: remote.full_name,
            description: remote.description.or_else(|| existing.description.clone()),
            language: remote.language.or_else(|| existing.language.clone()),
            default_branch: remote.default_branch,
            visibility: remote.visibility,
            is_fork: remote.is_fork,
            is_archived: remote.is_archived,
            size_kb: remote.size_kb.or(existing.size_kb),
            stars: remote.stars,
            pushed_at: remote.pushed_at.or(existing.pushed_at),
            updated_at: remote.updated_at,
            topics: remote.topics,
            clone_url_https: remote.clone_url_https,
            clone_url_ssh: remote.clone_url_ssh.or_else(|| existing.clone_url_ssh.clone()),

            // Preserved local state
            clone_status: existing.clone_status,
            local_path: existing.local_path.clone(),
            local_size_kb: existing.local_size_kb,
            last_synced_at: existing.last_synced_at,
            last_error: existing.last_error.clone(),
            behind_count: existing.behind_count,
            selected: existing.selected,
            deleted_on_remote: false,
            discovered_at: existing.discovered_at,
        }
    } else {
        RepoEntry {
            github_id: remote.github_id,
            owner_login: remote.owner_login,
            name: remote.name,
            full_name: remote.full_name,
            description: remote.description,
            language: remote.language,
            default_branch: remote.default_branch,
            visibility: remote.visibility,
            is_fork: remote.is_fork,
            is_archived: remote.is_archived,
            size_kb: remote.size_kb,
            stars: remote.stars,
            pushed_at: remote.pushed_at,
            updated_at: remote.updated_at,
            topics: remote.topics,
            clone_url_https: remote.clone_url_https,
            clone_url_ssh: remote.clone_url_ssh,
            clone_status: CloneStatus::NotCloned,
            local_path: None,
            local_size_kb: None,
            last_synced_at: None,
            last_error: None,
            behind_count: 0,
            selected: new, // default: select new repos
            deleted_on_remote: false,
            discovered_at: now,
        }
    }
}

/// Determine if a cached `RepoEntry` is in the scope of a given discovery source.
/// Scope rules (per git-vacuum-github-integration.md §5.3):
/// - MyRepos → only user's personal repos
/// - Org(login) → only that org's repos
/// - Starred → never prune (we don't track ownership for starred)
/// - All → everything
pub fn should_prune_from_scope(
    entry: &RepoEntry,
    source: &RepoSource,
) -> bool {
    if !is_in_scope(entry, source) {
        return false;
    }
    // We only prune from "all my owned/accessible" scopes.
    !matches!(source, RepoSource::Starred)
}

#[allow(dead_code)]
pub fn topics_to_json(topics: &[String]) -> String {
    serde_json::to_string(topics).unwrap_or_else(|_| "[]".to_string())
}

#[allow(dead_code)]
pub fn topics_from_json(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}
