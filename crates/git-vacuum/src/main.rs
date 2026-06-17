use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use git_vacuum_app::reduce;
use git_vacuum_app::state::{AppState, AuthScreenState, RunningAppState, TabKind};
use git_vacuum_app::App;
use git_vacuum_core::{
    Action, AppEvent, AuthMethod, EventBus, Effect, EventBusHandle, RepoSource, UserInfo,
};
use git_vacuum_db::SqliteDatabase;
use git_vacuum_core::Database as _;
use git_vacuum_git::Git2GitOps;
use git_vacuum_github::OctocrabGithubApi;
use git_vacuum_keyring::PlatformKeyring;
use git_vacuum_service::{run_sync as svc_run_sync, Services, SyncRequest};
use git_vacuum_tui::terminal;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures::StreamExt;

#[derive(Parser, Debug)]
#[command(name = "git-vacuum", about = "Local GitHub backup & mirror TUI")]
struct Args {
    /// GitHub Personal Access Token (alternative to keyring)
    #[arg(long, env = "GITHUB_TOKEN")]
    token: Option<String>,

    /// Skip the TUI and just sync (headless mode)
    #[arg(long)]
    sync: bool,

    /// Path to the database file
    #[arg(long, env = "GIT_VACUUM_DB")]
    db_path: Option<PathBuf>,

    /// Where to clone repos
    #[arg(long, env = "GIT_VACUUM_CLONE_PATH")]
    clone_path: Option<PathBuf>,

    /// Concurrent clone/fetch operations
    #[arg(long, default_value_t = 8)]
    concurrency: usize,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    // Resolve paths
    let db_path = args.db_path.clone().unwrap_or_else(default_db_path);
    let clone_path = args.clone_path.clone().unwrap_or_else(default_clone_path);

    // Initialize infrastructure
    let db = Arc::new(SqliteDatabase::open(&db_path)?);
    db.run_migrations()?;
    let github = Arc::new(OctocrabGithubApi::new(
        "https://api.github.com",
        format!("git-vacuum/{}", env!("CARGO_PKG_VERSION")),
    ));
    let git_ops = Arc::new(Git2GitOps::new());
    let keyring = Arc::new(PlatformKeyring::new()?);
    let services = Arc::new(Services::new(github, git_ops, db.clone(), keyring));

    // Non-interactive sync mode
    if args.sync {
        return run_headless_sync(services, args.token, clone_path, args.concurrency).await;
    }

    // TUI mode
    run_tui(services, db, clone_path, args.concurrency).await
}

fn default_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("git-vacuum")
        .join("db.sqlite")
}

fn default_clone_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("git-vacuum")
}

async fn run_headless_sync(
    services: Arc<Services>,
    token: Option<String>,
    clone_path: PathBuf,
    concurrency: usize,
) -> Result<()> {
    let token = token.ok_or_else(|| anyhow::anyhow!("--token required for --sync mode"))?;
    services.github.set_token(&token);
    let user = services.github.validate_token().await?;
    services.keyring.set_token(&token)?;
    services.db.upsert_account(&user)?;

    let repos = git_vacuum_service::run_discovery(services.clone(), RepoSource::MyRepos).await?;
    let selected: Vec<_> = repos.iter().filter(|r| r.selected).cloned().collect();
    if selected.is_empty() {
        println!("No repos selected. Exiting.");
        return Ok(());
    }
    println!("Syncing {} repos to {}", selected.len(), clone_path.display());

    let (progress_tx, _app_tx) = mpsc::unbounded_channel();
    let (app_tx2, _rx) = mpsc::unbounded_channel();
    let cancel_rx = services.github.set_token(&token); // dummy: we don't need cancel in headless
    drop(cancel_rx);
    let _ = svc_run_sync(
        services,
        SyncRequest {
            repos: selected,
            base_path: clone_path,
            concurrency,
            options: git_vacuum_core::SyncOptions::default(),
        },
        progress_tx,
        app_tx2,
        tokio::sync::watch::channel(false).1,
    ).await;
    Ok(())
}

async fn run_tui(
    services: Arc<Services>,
    db: Arc<SqliteDatabase>,
    clone_path: PathBuf,
    concurrency: usize,
) -> Result<()> {
    let (bus, handle) = EventBus::new();
    let mut app = App::new(services.clone(), handle);

    // Set the clone path into the running state
    if let AppState::Running(ref mut r) = app.state {
        r.clone_path = clone_path.to_string_lossy().to_string();
        r.tab_states.sync_center.concurrency = concurrency;
    }

    // Try loading stored credentials
    let services_for_load = services.clone();
    let bus_tx = bus.app_tx.clone();
    let user = git_vacuum_service::load_stored_credentials(services_for_load).await;
    match user {
        Ok(Some(info)) => {
            app.reduce_event(AppEvent::AuthSucceeded { info });
            let _ = db; // suppress unused
        }
        Ok(None) => {
            // Stay on auth screen
            let _ = bus_tx.send(AppEvent::OAuthCodeReceived {
                user_code: String::new(),
                verification_uri: String::new(),
                expires_in: Duration::from_secs(0),
            }); // no-op wakeup
        }
        Err(e) => {
            log::warn!("Stored credentials invalid: {e}");
        }
    }

    // Terminal
    let mut terminal = terminal::init()?;
    let result = run_loop(&mut terminal, &mut app, &bus).await;
    terminal::restore()?;
    result
}

async fn run_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    bus: &git_vacuum_core::EventBus,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(16));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // 1. Drain background AppEvents
        while let Ok(ev) = app.event_bus.app_rx.try_recv() {
            app.reduce_event(ev);
        }
        while let Ok(ev) = app.event_bus.progress_rx.try_recv() {
            app.reduce_event(ev);
        }

        // 2. Execute any pending effects
        let effects = app.drain_effects();
        for effect in effects {
            spawn_effect(effect, app, bus);
        }

        // 3. Render
        terminal.draw(|f| git_vacuum_tui::render(f, app))?;

        // 4. Handle input + tick
        tokio::select! {
            biased;
            _ = tick.tick() => {
                app.tick_count = app.tick_count.wrapping_add(1);
            }
            maybe_event = events.next() => {
                if let Some(Ok(CtEvent::Key(key))) = maybe_event {
                    if key.kind == KeyEventKind::Press {
                        if let Some(action) = key_to_action(key, app) {
                            app.reduce(action);
                        }
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn key_to_action(key: KeyEvent, app: &mut App) -> Option<Action> {
    // Global
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }
    match key.code {
        KeyCode::Char('q') => return Some(Action::Quit),
        KeyCode::Char('?') => return Some(Action::OpenHelp),
        KeyCode::Char(':') => return Some(Action::OpenCommandPalette),
        KeyCode::Esc => return Some(Action::DismissModal),
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                return Some(Action::PrevTab);
            }
            return Some(Action::NextTab);
        }
        KeyCode::BackTab => return Some(Action::PrevTab),
        _ => {}
    }

    let state_kind = match &app.state {
        AppState::Auth(_) => 0,
        AppState::Running(_) => 1,
        AppState::FatalError(_) => return Some(Action::Quit),
    };

    if state_kind == 0 {
        let token_input = if let AppState::Auth(a) = &app.state { a.token_input.clone() } else { String::new() };
        let loading = if let AppState::Auth(a) = &app.state { a.loading } else { false };
        key_to_action_auth(key, &token_input, loading)
    } else {
        key_to_action_running(key, app)
    }
}

fn key_to_action_auth(key: KeyEvent, token_input: &str, loading: bool) -> Option<Action> {
    if loading {
        // Ignore input while a request is in flight
        return None;
    }
    match key.code {
        KeyCode::Enter => Some(Action::AuthSubmitToken(token_input.to_string())),
        KeyCode::Backspace => {
            let mut s = token_input.to_string();
            s.pop();
            Some(Action::AuthTokenInputChanged(s))
        }
        KeyCode::Char(c) => {
            let mut s = token_input.to_string();
            s.push(c);
            Some(Action::AuthTokenInputChanged(s))
        }
        _ => None,
    }
}

fn key_to_action_running(key: KeyEvent, app: &mut App) -> Option<Action> {
    let AppState::Running(state) = &mut app.state else { return None };

    if state.command_palette.is_some() {
        return key_to_action_palette(key);
    }
    if !state.modal_stack.is_empty() {
        return match key.code {
            KeyCode::Esc => Some(Action::DismissModal),
            KeyCode::Enter => Some(Action::ConfirmModal),
            _ => None,
        };
    }

    match state.active_tab {
        TabKind::Dashboard => match key.code {
            KeyCode::Char('r') => Some(Action::DashboardRefreshStats),
            KeyCode::Char('s') => Some(Action::DashboardStartSync),
            KeyCode::Enter => Some(Action::DashboardStartSync),
            _ => None,
        },
        TabKind::Explorer => match key.code {
            KeyCode::Char(' ') => {
                let cur = state.tab_states.explorer.cursor;
                Some(Action::ExplorerToggle(cur))
            }
            KeyCode::Char('r') => Some(Action::ExplorerRefresh),
            KeyCode::Enter => Some(Action::ExplorerStartSync),
            KeyCode::Char('/') => Some(Action::ExplorerSetFilter(String::new())),
            KeyCode::Down => {
                let max = state.repos.len().saturating_sub(1);
                state.tab_states.explorer.cursor = (state.tab_states.explorer.cursor + 1).min(max);
                None
            }
            KeyCode::Up => {
                state.tab_states.explorer.cursor = state.tab_states.explorer.cursor.saturating_sub(1);
                None
            }
            _ => None,
        },
        TabKind::SyncCenter => match key.code {
            KeyCode::Char('p') => Some(Action::SyncPause),
            KeyCode::Char('r') => Some(Action::SyncResume),
            KeyCode::Char('c') => Some(Action::SyncCancel),
            _ => None,
        },
        TabKind::ActivityLog => match key.code {
            KeyCode::Char('r') => Some(Action::DashboardRefreshStats),
            _ => None,
        },
        TabKind::Settings => None,
    }
}

fn key_to_action_palette(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::CommandPaletteDismiss),
        KeyCode::Enter => Some(Action::CommandPaletteExecute(String::new())),
        KeyCode::Char(c) => Some(Action::CommandPaletteFilter(c.to_string())),
        KeyCode::Backspace => Some(Action::CommandPaletteFilter(String::new())),
        _ => None,
    }
}

/// Spawn an effect as a Tokio task.
/// This is where the binary translates high-level `Effect` enums into
/// actual service-layer calls and translates results back into `AppEvent`s.
fn spawn_effect(effect: Effect, app: &App, bus: &git_vacuum_core::EventBus) {
    let services = app.services.clone();
    let app_tx = bus.app_tx.clone();
    let progress_tx = bus.progress_tx.clone();
    let cancel_rx = bus.cancel_tx.subscribe();

    match effect {
        Effect::AuthenticatePat { token } => {
            tokio::spawn(async move {
                match git_vacuum_service::authenticate_pat(services, &token).await {
                    Ok(info) => {
                        let _ = app_tx.send(AppEvent::AuthSucceeded { info });
                    }
                    Err(e) => {
                        let _ = app_tx.send(AppEvent::AuthFailed {
                            reason: format!("{e:?}"),
                            detail: e.to_string(),
                        });
                    }
                }
            });
        }
        Effect::StartOAuthDeviceFlow { .. } => {
            let _ = app_tx.send(AppEvent::AuthFailed {
                reason: "not_implemented".into(),
                detail: "OAuth device flow is not yet implemented in MVP".into(),
            });
        }
        Effect::PollOAuthToken { .. } | Effect::CancelOAuth => { /* no-op */ }
        Effect::LoadStoredCredentials => {
            tokio::spawn(async move {
                if let Ok(Some(info)) = git_vacuum_service::load_stored_credentials(services).await {
                    let _ = app_tx.send(AppEvent::AuthSucceeded { info });
                }
            });
        }
        Effect::Logout => {
            tokio::spawn(async move {
                let _ = git_vacuum_service::logout(services).await;
                let _ = app_tx.send(AppEvent::LoggedOut);
            });
        }
        Effect::DiscoverRepos { source } => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            let source_for_event = source.clone();
            tokio::spawn(async move {
                match git_vacuum_service::run_discovery(services2, source).await {
                    Ok(entries) => {
                        // Push entries to the TUI immediately so it has data
                        let count = entries.len();
                        let _ = app_tx2.send(AppEvent::ReposLoaded { entries });
                        let _ = app_tx2.send(AppEvent::ReposDiscovered { source: source_for_event, count });
                    }
                    Err(e) => {
                        let _ = app_tx2.send(AppEvent::DiscoveryFailed { error: e.to_string() });
                    }
                }
            });
        }
        Effect::LoadReposFromDb => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            tokio::spawn(async move {
                if let Ok(rows) = services2.db.get_all_repos() {
                    let entries: Vec<git_vacuum_core::RepoEntry> = rows
                        .into_iter()
                        .map(|row| {
                            let visibility = match row.visibility.as_str() {
                                "private" => git_vacuum_core::RepoVisibility::Private,
                                "internal" => git_vacuum_core::RepoVisibility::Internal,
                                _ => git_vacuum_core::RepoVisibility::Public,
                            };
                            let topics: Vec<String> = row
                                .topics_json
                                .as_deref()
                                .and_then(|s| serde_json::from_str(s).ok())
                                .unwrap_or_default();
                            git_vacuum_core::RepoEntry {
                                github_id: row.github_id,
                                owner_login: row.owner,
                                name: row.name,
                                full_name: row.full_name,
                                description: row.description,
                                language: row.language,
                                default_branch: row.default_branch,
                                visibility,
                                is_fork: row.is_fork,
                                is_archived: row.is_archived,
                                size_kb: row.size_kb,
                                stars: row.stars,
                                pushed_at: row.pushed_at,
                                updated_at: row.updated_at,
                                topics,
                                clone_url_https: row.clone_url_https,
                                clone_url_ssh: row.clone_url_ssh,
                                clone_status: row.clone_status,
                                local_path: row.local_path,
                                local_size_kb: row.local_size_kb,
                                last_synced_at: row.last_synced_at,
                                last_error: row.last_error,
                                behind_count: row.behind_count,
                                selected: row.selected,
                                deleted_on_remote: row.deleted_on_remote,
                                discovered_at: row.discovered_at,
                            }
                        })
                        .collect();
                    let _ = app_tx2.send(AppEvent::ReposLoaded { entries });
                }
            });
        }
        Effect::PersistRepoSelection { github_ids, selected } => {
            let _ = services.db.set_repos_selected(&github_ids, selected);
        }
        Effect::StartSync { repos, base_path, concurrency } => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            let progress_tx2 = progress_tx.clone();
            let cancel_rx2 = cancel_rx.clone();
            tokio::spawn(async move {
                let _ = svc_run_sync(
                    services2,
                    SyncRequest {
                        repos,
                        base_path,
                        concurrency,
                        options: git_vacuum_core::SyncOptions::default(),
                    },
                    progress_tx2,
                    app_tx2,
                    cancel_rx2,
                ).await;
            });
        }
        Effect::PauseSync => {
            // For MVP: just send SyncPaused (real pause requires a flag
            // the dispatcher watches; out of scope here)
            let _ = app_tx.send(AppEvent::SyncPaused);
        }
        Effect::ResumeSync => {
            let _ = app_tx.send(AppEvent::SyncResumed);
        }
        Effect::CancelSync => {
            let _ = bus.cancel_tx.send(true);
            let _ = app_tx.send(AppEvent::SyncCancelled {
                summary: git_vacuum_core::PartialSyncSummary {
                    completed: 0, failed: 0, cancelled: 0, pending_dropped: 0, bytes_transferred: 0,
                },
            });
        }
        Effect::RefreshDashboardStats => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            tokio::spawn(async move {
                if let Ok(stats) = git_vacuum_service::compute_stats(services2).await {
                    let _ = app_tx2.send(AppEvent::StatsRefreshed);
                    // For MVP, send a follow-up to push the stats into the dashboard tab.
                    // (In a fuller impl, we'd send a dedicated DashboardStatsUpdated event.)
                    let _ = stats;
                }
            });
        }
        Effect::RecordSyncRun { .. } | Effect::ExportRun { .. } | Effect::SaveSetting { .. } | Effect::TestConnection | Effect::PersistRepos { .. } | Effect::MarkReposDeleted { .. } => {
            log::debug!("Effect not yet implemented: {effect:?}");
        }
        Effect::CloneSingle { .. } | Effect::SyncSingle { .. } => {
            log::debug!("Single-repo op not yet wired: {effect:?}");
        }
        Effect::None => {}
    }
    let _ = app_tx; let _ = progress_tx; let _ = cancel_rx;
    let _ = Instant::now();
    let _: AuthMethod = AuthMethod::Pat;
    let _: Option<UserInfo> = None;
}
