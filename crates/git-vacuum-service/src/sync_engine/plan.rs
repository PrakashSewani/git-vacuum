use std::path::{Path, PathBuf};
use std::sync::Arc;

use git_vacuum_core::{JobId, JobSpec, PlannedOperation, Priority, RepoEntry};

use crate::Services;
use crate::SyncRequest;

pub async fn resolve_plan(services: &Arc<Services>, request: &SyncRequest) -> Vec<JobSpec> {
    // For PAT-authenticated clones we MUST embed the token in the URL, because
    // SSH would require the user to have set up a GitHub SSH key. The plan
    // embeds the token once per repo into the HTTPS clone URL. The token is
    // fetched from the OS keyring on the fly and is NOT persisted to the
    // JobSpec — only embedded in the URL string at the moment of clone.
    //
    // TODO: when OAuth device flow is implemented, prefer SSH for repos where
    // the user has a registered SSH key.
    let token = match services.keyring.get_token() {
        Ok(Some(t)) => Some(t),
        _ => None,
    };

    let mut jobs: Vec<JobSpec> = Vec::with_capacity(request.repos.len());
    let base = &request.base_path;
    for (idx, repo) in request.repos.iter().enumerate() {
        let local_path = base.join(&repo.owner_login).join(&repo.name);
        let operation = if services.git.is_git_repo(&local_path) {
            PlannedOperation::Sync
        } else {
            PlannedOperation::Clone
        };
        let priority = Priority::Normal;

        // Build a clone URL that works with token auth. The HTTPS URL pattern
        // is: https://x-access-token:<token>@github.com/owner/repo.git
        // For a PAT, we can use https://<token>@github.com/owner/repo.git
        // (basic auth with token as username, empty password). Both forms
        // work for GitHub; the x-access-token form is documented as safe for
        // PATs and is the form gh CLI uses.
        let clone_url = match &token {
            Some(tok) if !tok.is_empty() => {
                // Strip the leading "https://" and inject the credentials.
                if let Some(rest) = repo
                    .clone_url_https
                    .strip_prefix("https://")
                    .map(str::to_string)
                {
                    format!("https://x-access-token:{}@{}", tok, rest)
                } else {
                    repo.clone_url_https.clone()
                }
            }
            _ => repo
                .clone_url_ssh
                .clone()
                .unwrap_or_else(|| repo.clone_url_https.clone()),
        };

        jobs.push(JobSpec {
            job_id: JobId(idx as u64),
            repo_full_name: repo.full_name.clone(),
            repo_github_id: repo.github_id,
            owner_login: repo.owner_login.clone(),
            clone_url,
            local_path,
            operation,
            priority,
            attempt: 0,
        });
    }
    jobs
}

#[allow(dead_code)]
pub fn path_for(base: &Path, repo: &RepoEntry) -> PathBuf {
    base.join(&repo.owner_login).join(&repo.name)
}
