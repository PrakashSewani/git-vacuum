use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use arboard::Clipboard;
use clap::Parser;
use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use futures::StreamExt;
use git_vacuum_app::state::{AppState, TabKind};
use git_vacuum_app::App;
use git_vacuum_core::Database as _;
use git_vacuum_core::{Action, AppEvent, AuthMethod, Effect, EventBus, RepoSource, UserInfo};
use git_vacuum_db::SqliteDatabase;
use git_vacuum_git::Git2GitOps;
use git_vacuum_github::OctocrabGithubApi;
use git_vacuum_keyring::PlatformKeyring;
use git_vacuum_service::{run_sync as svc_run_sync, Services, SyncRequest};
use git_vacuum_tui::terminal;
use ratatui::Terminal;
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(name = "git-vacuum", about = "Local GitHub backup & mirror TUI")]
struct Args {
    /// GitHub Personal Access Token (alternative to keyring)
    #[arg(long, env = "GITHUB_TOKEN")]
    token: Option<String>,

    /// Skip the TUI and just sync (headless mode)
    #[arg(long)]
    sync: bool,

    /// GitHub OAuth App client_id (required for browser sign-in)
    /// Register an OAuth App at https://github.com/settings/applications/new
    /// Set the callback URL to anything (device flow doesn't use it).
    #[arg(long, env = "GIT_VACUUM_OAUTH_CLIENT_ID")]
    oauth_client_id: Option<String>,

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
    // Load .env from the current working directory and the executable's
    // directory. Missing file is OK — environment vars set in the shell
    // already take precedence over .env values.
    let _ = dotenvy::dotenv();
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

    // TUI mode (pass initial token if provided via --token)
    run_tui(
        services,
        db,
        clone_path,
        args.concurrency,
        args.token,
        args.oauth_client_id,
    )
    .await
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
    println!(
        "Syncing {} repos to {}",
        selected.len(),
        clone_path.display()
    );

    let (progress_tx, _app_tx) = mpsc::unbounded_channel();
    let (app_tx2, _rx) = mpsc::unbounded_channel();
    services.github.set_token(&token); // dummy: we don't need cancel in headless
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
    )
    .await;
    Ok(())
}

async fn run_tui(
    services: Arc<Services>,
    _db: Arc<SqliteDatabase>,
    clone_path: PathBuf,
    concurrency: usize,
    initial_token: Option<String>,
    oauth_client_id: Option<String>,
) -> Result<()> {
    let (bus, handle) = EventBus::new();
    let mut app = App::new(services.clone(), handle, oauth_client_id.clone());

    // Set the clone path into the running state
    if let AppState::Running(ref mut r) = app.state {
        r.clone_path = clone_path.to_string_lossy().to_string();
        r.tab_states.sync_center.concurrency = concurrency;
    }

    // Try loading stored credentials
    let services_for_load = services.clone();

    // If --token was passed, store it in the keyring and treat as authenticated.
    if let Some(token) = initial_token.as_deref() {
        match git_vacuum_service::authenticate_pat(services.clone(), token).await {
            Ok(info) => {
                let _ = services.keyring.set_token(token);
                let _ = services.db.upsert_account(&info);
                app.reduce_event(AppEvent::AuthSucceeded { info });
            }
            Err(e) => {
                log::warn!("--token auth failed: {e}");
                // Fall through to load stored credentials or auth screen
            }
        }
    }

    let user = git_vacuum_service::load_stored_credentials(services_for_load).await;
    match user {
        Ok(Some(info)) => {
            app.reduce_event(AppEvent::AuthSucceeded { info });
        }
        Ok(None) => {
            // No stored credentials: stay on the auth screen. The default
            // AuthScreenState.phase is MethodPicker, so the user is asked
            // to choose PAT / OAuth / gh CLI on first launch.
        }
        Err(e) => {
            log::warn!("Stored credentials invalid: {e}");
        }
    }

    // Load saved settings into the running state.
    if let AppState::Running(ref mut r) = app.state {
        if let Ok(Some(v)) = services.db.get_setting("clone_path") {
            r.clone_path = v;
        }
        if let Ok(Some(v)) = services.db.get_setting("concurrency") {
            if let Ok(n) = v.parse::<usize>() {
                r.tab_states.sync_center.concurrency = n.clamp(1, 64);
            }
        }
        if let Ok(Some(v)) = services.db.get_setting("skip_archived") {
            r.tab_states.explorer.skip_archived = v == "true";
        }
        if let Ok(Some(v)) = services.db.get_setting("skip_forks") {
            r.tab_states.explorer.skip_forks = v == "true";
        }
        if let Ok(Some(v)) = services.db.get_setting("org_input") {
            r.tab_states.explorer.org_input = v;
        }
        if let Ok(Some(v)) = services.db.get_setting("topic_filter") {
            r.tab_states.explorer.topic_filter = v;
        }
        if let Ok(Some(v)) = services.db.get_setting("default_source") {
            r.tab_states.explorer.source = match v.as_str() {
                "Starred" => git_vacuum_core::RepoSource::Starred,
                "All" => git_vacuum_core::RepoSource::All,
                _ => git_vacuum_core::RepoSource::MyRepos,
            };
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
    let mut animation_tick = tokio::time::interval(Duration::from_millis(100));
    animation_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_animation_tick: u64 = 0;

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

        // 4. Handle input + ticks
        tokio::select! {
            biased;
            _ = animation_tick.tick() => {
                last_animation_tick = last_animation_tick.wrapping_add(1);
                app.tick_count = last_animation_tick;
                app.reduce_event(AppEvent::Tick);
            }
            _ = tick.tick() => {
                // Redraw tick (16ms). We don't increment tick_count here
                // so spinners stay at 100ms cadence; this tick is just for
                // keeping the input event loop responsive.
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(CtEvent::Key(key))) => {
                        if key.kind == KeyEventKind::Press {
                            // On Windows, intercept Ctrl+V to read the clipboard
                            // (Conhost does not support bracketed paste).
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'))
                            {
                                if let Some(text) = read_clipboard() {
                                    handle_paste(&text, app);
                                }
                            } else if let Some(action) = key_to_action(key, app) {
                                app.reduce(action);
                            }
                        }
                    }
                    Some(Ok(CtEvent::Paste(text))) => {
                        // Bracketed paste: insert the full text in one action
                        handle_paste(&text, app);
                    }
                    Some(Ok(CtEvent::Resize(_, _))) => {
                        // Ratatui handles resize automatically; no action needed
                    }
                    Some(Ok(CtEvent::FocusGained | CtEvent::FocusLost | CtEvent::Mouse(_))) => {
                        // Ignore focus/mouse events
                    }
                    Some(Err(_)) | None => {
                        // Stream closed or error: just continue
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn read_clipboard() -> Option<String> {
    // Try to acquire the clipboard. If the lock times out, skip silently.
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}

fn handle_paste(text: &str, app: &mut App) {
    // Strip whitespace, control chars, and the literal ^V (0x16) that some
    // terminals emit when bracketed paste is unavailable. Also strip common
    // shell-paste artifacts like the trailing newline Windows adds.
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();

    if cleaned.is_empty() {
        return;
    }

    match &mut app.state {
        AppState::Auth(auth) if !auth.loading => {
            let mut new_buf = auth.token_input.clone();
            new_buf.push_str(&cleaned);
            app.reduce(Action::AuthTokenInputChanged(new_buf));
        }
        AppState::Running(state) => {
            // Route to whatever input is focused: filter / org input / topic filter
            if state.command_palette.is_some() {
                if let Some(p) = state.command_palette.as_mut() {
                    let mut new_input = p.input.clone();
                    new_input.push_str(&cleaned);
                    app.reduce(Action::CommandPaletteFilter(new_input));
                }
            } else {
                // Explorer filter is the most common paste target
                let mut new_filter = state.tab_states.explorer.filter_text.clone();
                new_filter.push_str(&cleaned);
                app.reduce(Action::ExplorerSetFilter(new_filter));
            }
        }
        _ => {}
    }
}

fn key_to_action(key: KeyEvent, app: &mut App) -> Option<Action> {
    // Global
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }
    // In Auth state, Esc behavior depends on the current phase — we
    // route it through key_to_action_auth below by intercepting the
    // global DismissModal match.
    if let AppState::Auth(_) = &app.state {
        if key.code == KeyCode::Esc {
            return key_to_action_auth(key, app);
        }
    }
    if let AppState::Running(state) = &app.state {
        if state.active_tab == TabKind::Settings
            && key.code == KeyCode::Tab
            && !key.modifiers.contains(KeyModifiers::SHIFT)
        {
            let cats = git_vacuum_core::SettingsCategory::all();
            let current = cats
                .iter()
                .position(|c| *c == state.tab_states.settings.selected_category)
                .unwrap_or(0);
            let next = (current + 1) % cats.len();
            return Some(Action::SettingsSwitchCategory(next));
        }
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
        if let AppState::Auth(_) = &app.state {
            key_to_action_auth(key, app)
        } else {
            None
        }
    } else {
        key_to_action_running(key, app)
    }
}

fn key_to_action_auth(key: KeyEvent, app: &App) -> Option<Action> {
    let AppState::Auth(auth) = &app.state else {
        return None;
    };

    use git_vacuum_app::state::{AuthMethodChoice, AuthPhase};

    match auth.phase {
        AuthPhase::MethodPicker => {
            // Cursor navigation + selection.
            match key.code {
                KeyCode::Up => Some(Action::AuthMethodCursorMoved(-1)),
                KeyCode::Down => Some(Action::AuthMethodCursorMoved(1)),
                KeyCode::Char('1') => Some(Action::AuthMethodSelected(AuthMethodChoice::Pat)),
                KeyCode::Char('2') => Some(Action::AuthMethodSelected(AuthMethodChoice::OAuth)),
                KeyCode::Char('3') => Some(Action::AuthMethodSelected(AuthMethodChoice::GhCli)),
                KeyCode::Enter => {
                    let method = match auth.method_cursor {
                        0 => AuthMethodChoice::Pat,
                        1 => AuthMethodChoice::OAuth,
                        _ => AuthMethodChoice::GhCli,
                    };
                    Some(Action::AuthMethodSelected(method))
                }
                KeyCode::Esc | KeyCode::Char('q') => Some(Action::Quit),
                _ => None,
            }
        }
        AuthPhase::PatInput => {
            // Allow Esc to go back to the picker.
            if key.code == KeyCode::Esc {
                return Some(Action::AuthBackToMethodPicker);
            }
            // 'o' jumps to OAuth activation (preserved from the old UX).
            if matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O')) {
                return Some(Action::AuthStartOAuth);
            }
            // 'p' is a no-op (we're already in PAT input).
            if matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P')) {
                return Some(Action::AuthStartPAT);
            }
            // While a request is in-flight, swallow everything but Esc.
            if auth.loading {
                return None;
            }
            match key.code {
                KeyCode::Enter => Some(Action::AuthSubmitToken(auth.token_input.clone())),
                KeyCode::Backspace => {
                    let mut s = auth.token_input.clone();
                    s.pop();
                    Some(Action::AuthTokenInputChanged(s))
                }
                KeyCode::Char(c) => {
                    if c.is_control() || c == '\u{16}' {
                        return None;
                    }
                    let mut s = auth.token_input.clone();
                    s.push(c);
                    Some(Action::AuthTokenInputChanged(s))
                }
                _ => None,
            }
        }
        AuthPhase::Validating => {
            // During a validating request, allow Esc to cancel.
            if key.code == KeyCode::Esc {
                return Some(Action::AuthBackToMethodPicker);
            }
            None
        }
        AuthPhase::DeviceActivation => {
            if auth.show_url_prompt {
                if key.code == KeyCode::Esc {
                    return Some(Action::AuthDismissUrlPrompt);
                }
                if key.code == KeyCode::Enter
                    || matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O'))
                {
                    return Some(Action::AuthOpenOAuthUrl);
                }
                return None;
            }
            if key.code == KeyCode::Esc {
                return Some(Action::AuthBackToMethodPicker);
            }
            if key.code == KeyCode::Enter
                || matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O'))
            {
                return Some(Action::AuthOpenOAuthUrl);
            }
            if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
                return Some(Action::AuthCopyOAuthCode);
            }
            None
        }
        AuthPhase::AuthFailed => {
            match key.code {
                KeyCode::Esc => Some(Action::AuthBackToMethodPicker),
                KeyCode::Tab | KeyCode::Right => Some(Action::AuthFailedFocusMoved(1)),
                KeyCode::BackTab | KeyCode::Left => Some(Action::AuthFailedFocusMoved(-1)),
                KeyCode::Enter => {
                    if auth.failed_focus == 0 {
                        // Try Again — re-activate the last method.
                        Some(Action::AuthMethodSelected(auth.last_method))
                    } else {
                        // Pick a different method.
                        Some(Action::AuthBackToMethodPicker)
                    }
                }
                _ => None,
            }
        }
    }
}

// Convenience alias for borrowing the AuthScreenState without taking ownership.

fn key_to_action_running(key: KeyEvent, app: &mut App) -> Option<Action> {
    let AppState::Running(state) = &mut app.state else {
        return None;
    };

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

    // Welcome screen: any keypress dismisses it (we don't forward the key)
    if state.welcome_state.is_some() {
        return Some(Action::DismissWelcome);
    }

    // Number keys jump directly to a tab
    if let KeyCode::Char(c) = key.code {
        if let Some(n) = c.to_digit(10) {
            if (1..=5).contains(&n) {
                return Some(Action::SwitchTabByNumber(n as u8));
            }
        }
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
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::ExplorerSelectAll)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::ExplorerDeselectAll)
            }
            KeyCode::Down => {
                let max = state.repos.len().saturating_sub(1);
                state.tab_states.explorer.cursor = (state.tab_states.explorer.cursor + 1).min(max);
                None
            }
            KeyCode::Up => {
                state.tab_states.explorer.cursor =
                    state.tab_states.explorer.cursor.saturating_sub(1);
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
        TabKind::Settings => {
            let st = &mut state.tab_states.settings;
            if st.editing_field.is_some() {
                match key.code {
                    KeyCode::Esc => Some(Action::SettingsDiscard),
                    KeyCode::Enter => Some(Action::SettingsSave),
                    KeyCode::Backspace => {
                        st.draft_value.pop();
                        None
                    }
                    KeyCode::Char(c) => {
                        st.draft_value.push(c);
                        None
                    }
                    _ => None,
                }
            } else {
                match key.code {
                    KeyCode::Tab => Some(Action::NextTab),
                    KeyCode::BackTab => Some(Action::PrevTab),
                    KeyCode::Up => {
                        let new = st.selected_field.saturating_sub(1);
                        Some(Action::SettingsNavigate(new))
                    }
                    KeyCode::Down => {
                        let max = st.fields.len().saturating_sub(1);
                        let new = (st.selected_field + 1).min(max);
                        Some(Action::SettingsNavigate(new))
                    }
                    KeyCode::Enter => Some(Action::SettingsEdit(st.selected_field)),
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::SettingsSave)
                    }
                    KeyCode::Esc => Some(Action::SettingsDiscard),
                    _ => None,
                }
            }
        }
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
        Effect::StartOAuthDeviceFlow {
            client_id: _,
            scopes: _,
        } => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            let client_id_owned = app.oauth_client_id.clone().unwrap_or_default();
            tokio::spawn(async move {
                match git_vacuum_service::start_oauth_device_flow(services2, &client_id_owned).await
                {
                    Ok(init) => {
                        let _ = app_tx2.send(AppEvent::OAuthCodeReceived {
                            user_code: init.user_code,
                            verification_uri: init.verification_uri,
                            expires_in: init.expires_in,
                        });
                        // Spawn a poller that fires every 5s
                        let services3 = services.clone();
                        let app_tx3 = app_tx2.clone();
                        let device_code = init.device_code;
                        let interval_secs = init.interval.as_secs().max(5);
                        let expires_in = init.expires_in;
                        let client_id_for_poll = client_id_owned.clone();
                        tokio::spawn(async move {
                            let start = std::time::Instant::now();
                            loop {
                                tokio::time::sleep(std::time::Duration::from_secs(interval_secs))
                                    .await;
                                if start.elapsed() > expires_in {
                                    let _ = app_tx3.send(AppEvent::OAuthTimeout);
                                    break;
                                }
                                match git_vacuum_service::poll_oauth_device_flow(
                                    services3.clone(),
                                    &client_id_for_poll,
                                    device_code.clone(),
                                )
                                .await
                                {
                                    Ok(git_vacuum_core::DeviceFlowPoll::Success {
                                        access_token,
                                        scopes,
                                    }) => {
                                        let _ = app_tx3.send(AppEvent::OAuthTokenReceived {
                                            token: access_token,
                                            scopes,
                                        });
                                        break;
                                    }
                                    Ok(git_vacuum_core::DeviceFlowPoll::SlowDown {
                                        new_interval,
                                    }) => {
                                        // Use new interval next iteration
                                        let extra =
                                            new_interval.as_secs().saturating_sub(interval_secs);
                                        if extra > 0 {
                                            tokio::time::sleep(std::time::Duration::from_secs(
                                                extra,
                                            ))
                                            .await;
                                        }
                                    }
                                    Ok(git_vacuum_core::DeviceFlowPoll::Expired) => {
                                        let _ = app_tx3.send(AppEvent::OAuthTimeout);
                                        break;
                                    }
                                    Ok(git_vacuum_core::DeviceFlowPoll::AccessDenied) => {
                                        let _ = app_tx3.send(AppEvent::AuthFailed {
                                            reason: "access_denied".into(),
                                            detail: "OAuth authorization was denied".into(),
                                        });
                                        break;
                                    }
                                    Ok(git_vacuum_core::DeviceFlowPoll::Pending) => {
                                        // Keep polling
                                    }
                                    Err(e) => {
                                        // Network blip or OAuth error. Show in
                                        // the TUI; if it's a config error (bad
                                        // client_id etc.) the user needs to know.
                                        let msg = format!("{e}");
                                        let permanent = msg.contains("client_id")
                                            || msg.contains("HTTP 4")
                                            || msg.contains("invalid_client")
                                            || msg.contains("incorrect_client");
                                        if permanent {
                                            let _ = app_tx3.send(AppEvent::AuthFailed {
                                                reason: "oauth_poll_failed".into(),
                                                detail: msg,
                                            });
                                            break;
                                        } else {
                                            log::warn!("OAuth poll error (will retry): {e}");
                                        }
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        let _ = app_tx2.send(AppEvent::AuthFailed {
                            reason: "oauth_init_failed".into(),
                            detail: format!("Could not start OAuth flow: {e}"),
                        });
                    }
                }
            });
        }
        Effect::PollOAuthToken {
            client_id: _,
            device_code: _,
            interval,
        } => {
            // Deprecated: handled inline in StartOAuthDeviceFlow
            let _ = interval;
        }
        Effect::CancelOAuth => {
            // No-op: the poller exits naturally when it can't find the device
            // code (after deletion) or on user action. We don't expose a
            // cancel endpoint in the github API.
        }
        Effect::OpenUrl { url } => {
            let url_owned = url.clone();
            tokio::spawn(async move {
                if let Err(e) = tokio::task::spawn_blocking(move || open::that(&url_owned)).await {
                    log::warn!("Could not open browser URL: {e}");
                }
            });
        }
        Effect::CopyToClipboard { text } => {
            let text_owned = text.clone();
            tokio::spawn(async move {
                let _ = tokio::task::spawn_blocking(move || {
                    Clipboard::new().and_then(|mut cb| cb.set_text(text_owned))
                })
                .await;
            });
        }
        Effect::CompleteOAuthWithToken { token } => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            tokio::spawn(async move {
                match git_vacuum_service::complete_oauth_with_token(services2, token).await {
                    Ok(info) => {
                        let _ = app_tx2.send(AppEvent::AuthSucceeded { info });
                    }
                    Err(e) => {
                        let _ = app_tx2.send(AppEvent::AuthFailed {
                            reason: "oauth_validate_failed".into(),
                            detail: format!("OAuth token invalid: {e}"),
                        });
                    }
                }
            });
        }
        Effect::LoadStoredCredentials => {
            tokio::spawn(async move {
                if let Ok(Some(info)) = git_vacuum_service::load_stored_credentials(services).await
                {
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
                        let _ = app_tx2.send(AppEvent::ReposDiscovered {
                            source: source_for_event,
                            count,
                        });
                    }
                    Err(e) => {
                        let _ = app_tx2.send(AppEvent::DiscoveryFailed {
                            error: e.to_string(),
                        });
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
        Effect::PersistRepoSelection {
            github_ids,
            selected,
        } => {
            let _ = services.db.set_repos_selected(&github_ids, selected);
        }
        Effect::StartSync {
            repos,
            base_path,
            concurrency,
        } => {
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
                )
                .await;
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
                    completed: 0,
                    failed: 0,
                    cancelled: 0,
                    pending_dropped: 0,
                    bytes_transferred: 0,
                },
            });
        }
        Effect::RefreshDashboardStats => {
            let services2 = services.clone();
            let app_tx2 = app_tx.clone();
            tokio::spawn(async move {
                let stats = git_vacuum_service::compute_stats(services2.clone()).await;
                let attention = services2.db.get_attention_list(10).unwrap_or_default();
                match stats {
                    Ok(s) => {
                        let _ = app_tx2.send(AppEvent::DashboardStatsUpdated {
                            stats: s,
                            attention,
                        });
                    }
                    Err(e) => {
                        // Even on failure, send a zeroed stats to clear loading state
                        // so the UI doesn't stay stuck on "Loading stats..."
                        log::warn!("Failed to compute dashboard stats: {e}");
                        let _ = app_tx2.send(AppEvent::DashboardStatsUpdated {
                            stats: git_vacuum_core::DashboardStats {
                                total_repos: 0,
                                up_to_date: 0,
                                behind: 0,
                                errors: 0,
                                total_size_bytes: 0,
                            },
                            attention: vec![],
                        });
                    }
                }
            });
        }
        Effect::SaveSetting { key, value } => {
            tokio::spawn(async move {
                if let Err(e) = services.db.set_setting(&key, &value) {
                    log::warn!("Failed to save setting {}: {}", key, e);
                }
            });
        }
        Effect::RecordSyncRun { .. }
        | Effect::ExportRun { .. }
        | Effect::TestConnection
        | Effect::PersistRepos { .. }
        | Effect::MarkReposDeleted { .. } => {
            log::debug!("Effect not yet implemented: {effect:?}");
        }
        Effect::CloneSingle { .. } | Effect::SyncSingle { .. } => {
            log::debug!("Single-repo op not yet wired: {effect:?}");
        }
        Effect::None => {}
    }
    let _ = app_tx;
    let _ = progress_tx;
    let _ = cancel_rx;
    let _ = Instant::now();
    let _: AuthMethod = AuthMethod::Pat;
    let _: Option<UserInfo> = None;
}
