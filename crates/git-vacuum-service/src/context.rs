use std::sync::Arc;

use git_vacuum_core::traits::{Database, GitOps, GithubApi, KeyringStore};

pub struct Services {
    pub github: Arc<dyn GithubApi>,
    pub git: Arc<dyn GitOps>,
    pub db: Arc<dyn Database>,
    pub keyring: Arc<dyn KeyringStore>,
}
