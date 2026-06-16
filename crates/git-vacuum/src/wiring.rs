use std::path::PathBuf;
use std::sync::Arc;

use git_vacuum_app::{App, AppConfig};
use git_vacuum_core::EventBus;
use git_vacuum_db::{ConnectionPool, SqliteDatabase};
use git_vacuum_github::OctocrabGithubApi;
use git_vacuum_git::Git2Ops;
use git_vacuum_keyring::PlatformKeyring;
use git_vacuum_service::Services;

use crate::config::Cli;

pub struct AppContext {
    pub app: App,
    pub services: Arc<Services>,
    pub db_pool: ConnectionPool,
}

pub async fn create_app(cli: &Cli) -> Result<AppContext, String> {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("git-vacuum");

    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Cannot create data dir: {}", e))?;

    let db_path = data_dir.join("git-vacuum.db");
    let pool = ConnectionPool::open(&db_path)?;
    git_vacuum_db::migrations::run_migrations(&pool).await?;

    let db = Arc::new(SqliteDatabase::new(pool.clone()));
    let keyring = Arc::new(PlatformKeyring::new());
    let git = Arc::new(Git2Ops::new());

    let user_agent = format!("git-vacuum/{}", env!("CARGO_PKG_VERSION"));
    let github = Arc::new(OctocrabGithubApi::new(
        cli.github_url.clone(),
        user_agent,
    )?);

    let services = Arc::new(Services { github, git, db, keyring });

    let clone_path = cli.path.clone()
        .unwrap_or_else(|| dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("git-vacuum")
            .to_string_lossy()
            .to_string());

    let app_config = AppConfig {
        clone_path,
        default_concurrency: cli.concurrency.unwrap_or(8),
        github_base_url: cli.github_url.clone(),
        user_agent: format!("git-vacuum/{}", env!("CARGO_PKG_VERSION")),
    };

    let event_bus = EventBus::new();
    let app = App::new(app_config, event_bus);

    Ok(AppContext { app, services, db_pool: pool })
}
