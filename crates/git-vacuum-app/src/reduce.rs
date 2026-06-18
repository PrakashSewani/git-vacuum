//! The reducer — the only functions that mutate `App`.
//! These are pure-ish: they take `&mut App` and return `Vec<Effect>`.
//! They never `.await` — async work happens in spawned effect tasks.

use git_vacuum_core::{Action, AppEvent, AuthMethodChoice, Effect, JobId, RepoSource, SettingsCategory, TabTarget};
use git_vacuum_service::{authenticate_pat, load_stored_credentials, logout, run_discovery};

use crate::modals::{CommandAction, CommandPaletteState, Modal};
use crate::state::{
    AppState, AuthErrorCategory, AuthErrorView, AuthMode, AuthPhase, AuthScreenState,
    RunningAppState, TabKind, WelcomePhase,
};
use crate::tabs::LogStatus;
use crate::App;

pub fn reduce_action(app: &mut App, action: Action) -> Vec<Effect> {
    match action {
        Action::Quit => {
            app.should_quit = true;
            vec![]
        }
        Action::NoOp => vec![],
        _ => reduce_in_state(app, action),
    }
}

fn reduce_in_state(app: &mut App, action: Action) -> Vec<Effect> {
    match &app.state {
        AppState::Auth(_) => reduce_auth(app, action),
        AppState::Running(_) => reduce_running(app, action),
        AppState::FatalError(_) => vec![], // no actions accepted in fatal state
    }
}

fn reduce_auth(app: &mut App, action: Action) -> Vec<Effect> {
    let AppState::Auth(auth) = &mut app.state else { return vec![] };
    match action {
        Action::AuthMethodCursorMoved(delta) => {
            // 3 methods, wrap. GhCli is shown disabled; cursor still moves
            // so the user can see it, but selection is a no-op.
            let total = 3i8;
            let mut cur = auth.method_cursor as i8 + delta;
            if cur < 0 {
                cur += total;
            } else if cur >= total {
                cur -= total;
            }
            auth.method_cursor = cur as u8;
            vec![]
        }
        Action::AuthFailedFocusMoved(delta) => {
            // 2 buttons: 0 = Try Again, 1 = Pick a different method.
            let total = 2i8;
            let mut cur = auth.failed_focus as i8 + delta;
            if cur < 0 {
                cur += total;
            } else if cur >= total {
                cur -= total;
            }
            auth.failed_focus = cur as u8;
            vec![]
        }
        Action::AuthMethodSelected(method) => {
            match method {
                AuthMethodChoice::Pat => {
                    auth.method = AuthMethodChoice::Pat;
                    auth.last_method = AuthMethodChoice::Pat;
                    auth.mode = AuthMode::Pat;
                    auth.phase = AuthPhase::PatInput;
                    auth.error = None;
                }
                AuthMethodChoice::OAuth => {
                    auth.method = AuthMethodChoice::OAuth;
                    auth.last_method = AuthMethodChoice::OAuth;
                    auth.mode = AuthMode::OAuth;
                    auth.error = None;
                    if auth.oauth_client_id.as_deref().unwrap_or("").is_empty() {
                        auth.error = Some(AuthErrorView {
                            category: AuthErrorCategory::OAuthConfig,
                            headline: "OAuth requires a client_id".into(),
                            detail: "Register an OAuth App at https://github.com/settings/applications/new and pass --oauth-client-id <id> or set GIT_VACUUM_OAUTH_CLIENT_ID.".into(),
                            hints: vec![
                                "Or use a Personal Access Token (Esc to go back).".into(),
                            ],
                        });
                        auth.phase = AuthPhase::AuthFailed;
                        return vec![];
                    }
                    auth.loading = true;
                    auth.phase = AuthPhase::Validating;
                    return vec![Effect::StartOAuthDeviceFlow {
                        client_id: auth.oauth_client_id.clone().unwrap_or_default(),
                        scopes: vec!["repo".into(), "read:org".into(), "user".into()],
                    }];
                }
                AuthMethodChoice::GhCli => {
                    // No service plumbing in this iteration; show an error
                    // hint on the picker.
                    auth.error = Some(AuthErrorView {
                        category: AuthErrorCategory::Other,
                        headline: "gh CLI integration coming soon".into(),
                        detail: "git-vacuum can read tokens from the gh CLI, but this flow isn't wired up in the current build.".into(),
                        hints: vec![
                            "Use a Personal Access Token for now.".into(),
                            "OAuth Device Flow is also available (requires a client_id).".into(),
                        ],
                    });
                }
            }
            vec![]
        }
        Action::AuthBackToMethodPicker => {
            auth.phase = AuthPhase::MethodPicker;
            auth.loading = false;
            auth.error = None;
            auth.failed_focus = 0;
            vec![]
        }
        Action::AuthSubmitToken(token) => {
            // Sync the buffer to state and submit
            auth.token_input = token.clone();
            if token.is_empty() {
                auth.error = Some(AuthErrorView {
                    category: AuthErrorCategory::Other,
                    headline: "Token cannot be empty".into(),
                    detail: "Paste a GitHub Personal Access Token (starts with ghp_ or github_pat_).".into(),
                    hints: Vec::new(),
                });
                auth.phase = AuthPhase::AuthFailed;
                return vec![];
            }
            auth.loading = true;
            auth.error = None;
            auth.phase = AuthPhase::Validating;
            vec![Effect::AuthenticatePat { token }]
        }
        Action::AuthTokenInputChanged(s) => {
            // Just update the local input buffer; no side effects
            // Cap at 200 chars to prevent runaway paste flood
            auth.token_input = if s.chars().count() > 200 {
                s.chars().take(200).collect()
            } else {
                s
            };
            // Clear any stale error so it doesn't linger as the user types
            if auth.error.is_some() {
                auth.error = None;
            }
            // Going from empty to non-empty: leave them in PatInput
            vec![]
        }
        Action::AuthStartPAT => {
            auth.method = AuthMethodChoice::Pat;
            auth.mode = AuthMode::Pat;
            auth.phase = AuthPhase::PatInput;
            auth.error = None;
            vec![]
        }
        Action::AuthStartOAuth => {
            // 'o' shortcut from PAT input: immediately start OAuth if configured.
            auth.method = AuthMethodChoice::OAuth;
            auth.last_method = AuthMethodChoice::OAuth;
            auth.mode = AuthMode::OAuth;
            auth.error = None;
            if auth.oauth_client_id.as_deref().unwrap_or("").is_empty() {
                auth.error = Some(AuthErrorView {
                    category: AuthErrorCategory::OAuthConfig,
                    headline: "OAuth requires a client_id".into(),
                    detail: "Register an OAuth App at https://github.com/settings/applications/new and pass --oauth-client-id <id> or set GIT_VACUUM_OAUTH_CLIENT_ID.".into(),
                    hints: vec![
                        "Or use a Personal Access Token (Esc to go back).".into(),
                    ],
                });
                auth.phase = AuthPhase::AuthFailed;
                return vec![];
            }
            auth.loading = true;
            auth.phase = AuthPhase::Validating;
            return vec![Effect::StartOAuthDeviceFlow {
                client_id: auth.oauth_client_id.clone().unwrap_or_default(),
                scopes: vec!["repo".into(), "read:org".into(), "user".into()],
            }];
        }
        Action::AuthStartOAuthNow => {
            // Enter in the device activation screen now opens the browser URL.
            if let Some(oauth) = auth.oauth.as_ref() {
                auth.show_url_prompt = false;
                return vec![Effect::OpenUrl { url: oauth.verification_uri.clone() }];
            }
            vec![]
        }
        Action::AuthOpenOAuthUrl => {
            if let Some(oauth) = auth.oauth.as_ref() {
                auth.show_url_prompt = false;
                return vec![Effect::OpenUrl { url: oauth.verification_uri.clone() }];
            }
            vec![]
        }
        Action::AuthCopyOAuthCode => {
            if let Some(oauth) = auth.oauth.as_ref() {
                return vec![Effect::CopyToClipboard { text: oauth.user_code.clone() }];
            }
            vec![]
        }
        Action::AuthDismissUrlPrompt => {
            auth.show_url_prompt = false;
            vec![]
        }
        Action::AuthCancelOAuth => vec![],
        Action::AuthSkipForPublic => {
            // MVP: we still require auth, so just show a structured error
            // on the picker explaining why.
            auth.error = Some(AuthErrorView {
                category: AuthErrorCategory::Other,
                headline: "Authentication is required".into(),
                detail: "Public-only browsing is not supported in this build.".into(),
                hints: vec![
                    "Pick Personal Access Token or OAuth Device Flow to continue.".into(),
                ],
            });
            auth.phase = AuthPhase::AuthFailed;
            vec![]
        }
        _ => vec![],
    }
}

fn reduce_running(app: &mut App, action: Action) -> Vec<Effect> {
    let AppState::Running(state) = &mut app.state else { return vec![] };

    // Modal actions get priority
    if !state.modal_stack.is_empty() {
        return reduce_modal(app, action);
    }
    if state.command_palette.is_some() {
        return reduce_command_palette(app, action);
    }

    match action {
        // Global
        Action::Quit => { app.should_quit = true; vec![] }
        Action::OpenHelp => {
            state.modal_stack.push(Modal::Help(crate::modals::HelpModal { scroll: 0 }));
            vec![]
        }
        Action::OpenCommandPalette => {
            state.command_palette = Some(CommandPaletteState::open());
            vec![]
        }
        Action::SwitchTab(t) => {
            state.active_tab = tab_target_to_kind(t);
            if matches!(state.active_tab, TabKind::Settings) {
                rebuild_settings_fields(state);
            }
            vec![]
        }
        Action::NextTab => { state.active_tab = state.active_tab.next(); vec![] }
        Action::PrevTab => { state.active_tab = state.active_tab.prev(); vec![] }
        Action::SwitchTabByNumber(n) => {
            let tabs = TabKind::all();
            if n >= 1 && (n as usize) <= tabs.len() {
                state.active_tab = tabs[(n - 1) as usize];
                if matches!(state.active_tab, TabKind::Settings) {
                    rebuild_settings_fields(state);
                }
            }
            vec![]
        }
        Action::DismissWelcome => {
            state.welcome_state = None;
            vec![]
        }
        Action::DismissModal => {
            state.modal_stack.pop();
            vec![]
        }
        Action::ConfirmModal => {
            // For help modal, dismiss on confirm
            if let Some(Modal::Help(_)) = state.modal_stack.last() {
                state.modal_stack.pop();
            }
            vec![]
        }

        // Explorer
        Action::ExplorerRefresh => {
            state.tab_states.explorer.loading = true;
            vec![Effect::DiscoverRepos { source: state.tab_states.explorer.source.clone() }]
        }
        Action::ExplorerToggle(idx) => {
            if let Some(r) = state.repos.get_mut(idx) {
                r.selected = !r.selected;
                let id = r.github_id;
                let selected = r.selected;
                vec![Effect::PersistRepoSelection { github_ids: vec![id], selected }]
            } else {
                vec![]
            }
        }
        Action::ExplorerSelectAll => {
            for r in state.repos.iter_mut() { r.selected = true; }
            let ids: Vec<i64> = state.repos.iter().map(|r| r.github_id).collect();
            vec![Effect::PersistRepoSelection { github_ids: ids, selected: true }]
        }
        Action::ExplorerDeselectAll => {
            for r in state.repos.iter_mut() { r.selected = false; }
            let ids: Vec<i64> = state.repos.iter().map(|r| r.github_id).collect();
            vec![Effect::PersistRepoSelection { github_ids: ids, selected: false }]
        }
        Action::ExplorerSetFilter(s) => { state.tab_states.explorer.filter_text = s; vec![] }
        Action::ExplorerClearFilter => { state.tab_states.explorer.filter_text.clear(); vec![] }
        Action::ExplorerSetOrgInput(s) => { state.tab_states.explorer.org_input = s; vec![] }
        Action::ExplorerSetTopicFilter(s) => { state.tab_states.explorer.topic_filter = s; vec![] }
        Action::ExplorerToggleSkipArchived => { state.tab_states.explorer.skip_archived = !state.tab_states.explorer.skip_archived; vec![] }
        Action::ExplorerToggleSkipForks => { state.tab_states.explorer.skip_forks = !state.tab_states.explorer.skip_forks; vec![] }
        Action::ExplorerStartSync => {
            let selected: Vec<_> = state.repos.iter().filter(|r| r.selected).cloned().collect();
            if selected.is_empty() {
                return vec![];
            }
            let base = if state.clone_path.is_empty() {
                dirs_next_default()
            } else {
                std::path::PathBuf::from(&state.clone_path)
            };
            state.tab_states.sync_center.phase = crate::tabs::SyncPhase::PreSync;
            state.tab_states.sync_center.queued_repos = selected.len();
            state.tab_states.sync_center.base_path = base.clone();
            state.active_tab = TabKind::SyncCenter;
            vec![Effect::StartSync { repos: selected, base_path: base, concurrency: 8 }]
        }
        Action::DashboardStartSync => {
            let selected: Vec<_> = state.repos.iter().filter(|r| r.selected).cloned().collect();
            if selected.is_empty() {
                vec![]
            } else {
                let base = if state.clone_path.is_empty() {
                    dirs_next_default()
                } else {
                    std::path::PathBuf::from(&state.clone_path)
                };
                state.tab_states.sync_center.phase = crate::tabs::SyncPhase::PreSync;
                state.tab_states.sync_center.queued_repos = selected.len();
                state.active_tab = TabKind::SyncCenter;
                vec![Effect::StartSync { repos: selected, base_path: base, concurrency: 8 }]
            }
        }
        Action::DashboardRefreshStats => {
            vec![Effect::RefreshDashboardStats]
        }
        Action::SyncStart => {
            let selected: Vec<_> = state.repos.iter().filter(|r| r.selected).cloned().collect();
            if selected.is_empty() {
                return vec![];
            }
            let base = if state.clone_path.is_empty() {
                dirs_next_default()
            } else {
                std::path::PathBuf::from(&state.clone_path)
            };
            state.tab_states.sync_center.phase = crate::tabs::SyncPhase::Active;
            state.tab_states.sync_center.queued_repos = selected.len();
            vec![Effect::StartSync { repos: selected, base_path: base, concurrency: state.tab_states.sync_center.concurrency.max(1) }]
        }
        Action::SyncCancel => vec![Effect::CancelSync],
        Action::SyncPause => vec![Effect::PauseSync],
        Action::SyncResume => vec![Effect::ResumeSync],
        Action::SettingsSwitchCategory(idx) => {
            let cats = git_vacuum_core::SettingsCategory::all();
            if idx < cats.len() {
                state.tab_states.settings.selected_category = cats[idx];
                state.tab_states.settings.selected_field = 0;
                state.tab_states.settings.editing_field = None;
                state.tab_states.settings.draft_value.clear();
                state.tab_states.settings.has_unsaved_changes = false;
                rebuild_settings_fields(state);
            }
            vec![]
        }
        Action::SettingsNavigate(idx) => {
            state.tab_states.settings.selected_field = idx.min(state.tab_states.settings.fields.len().saturating_sub(1));
            vec![]
        }
        Action::SettingsEdit(idx) => {
            if let Some(field) = state.tab_states.settings.fields.get(idx).cloned() {
                match field.kind {
                    git_vacuum_core::SettingsFieldKind::Boolean => {
                        let new_val = if field.value == "true" { "false" } else { "true" };
                        if let Some(f) = state.tab_states.settings.fields.get_mut(idx) {
                            f.value = new_val.to_string();
                        }
                        state.tab_states.settings.has_unsaved_changes = true;
                        apply_setting_value(state, &field.key, &new_val);
                    }
                    git_vacuum_core::SettingsFieldKind::Dropdown { options } => {
                        let current = options.iter().position(|o| o == &field.value).unwrap_or(0);
                        let next = (current + 1) % options.len();
                        let new_val = options[next].clone();
                        if let Some(f) = state.tab_states.settings.fields.get_mut(idx) {
                            f.value = new_val.clone();
                        }
                        state.tab_states.settings.has_unsaved_changes = true;
                        apply_setting_value(state, &field.key, &new_val);
                    }
                    _ => {
                        state.tab_states.settings.editing_field = Some(idx);
                        state.tab_states.settings.draft_value = field.value.clone();
                    }
                }
            }
            vec![]
        }
        Action::SettingsToggle(idx) => {
            if let Some(field) = state.tab_states.settings.fields.get(idx).cloned() {
                if matches!(field.kind, git_vacuum_core::SettingsFieldKind::Boolean) {
                    let new_val = if field.value == "true" { "false" } else { "true" };
                    if let Some(f) = state.tab_states.settings.fields.get_mut(idx) {
                        f.value = new_val.to_string();
                    }
                    state.tab_states.settings.has_unsaved_changes = true;
                    apply_setting_value(state, &field.key, &new_val);
                }
            }
            vec![]
        }
        Action::SettingsSelectDropdown(idx) => {
            if let Some(field) = state.tab_states.settings.fields.get(idx).cloned() {
                if let git_vacuum_core::SettingsFieldKind::Dropdown { options } = field.kind {
                    let current = options.iter().position(|o| o == &field.value).unwrap_or(0);
                    let next = (current + 1) % options.len();
                    let new_val = options[next].clone();
                    if let Some(f) = state.tab_states.settings.fields.get_mut(idx) {
                        f.value = new_val.clone();
                    }
                    state.tab_states.settings.has_unsaved_changes = true;
                    apply_setting_value(state, &field.key, &new_val);
                }
            }
            vec![]
        }
        Action::SettingsDropdownPick(_) => vec![],
        Action::SettingsSave => {
            if let Some(idx) = state.tab_states.settings.editing_field {
                let key = state.tab_states.settings.fields.get(idx).map(|f| f.key.clone()).unwrap_or_default();
                let value = state.tab_states.settings.draft_value.clone();
                if let Some(field) = state.tab_states.settings.fields.get_mut(idx) {
                    field.value = value.clone();
                }
                apply_setting_value(state, &key, &value);
                state.tab_states.settings.editing_field = None;
                state.tab_states.settings.draft_value.clear();
            }
            state.tab_states.settings.has_unsaved_changes = false;
            let mut effects = Vec::new();
            for field in &state.tab_states.settings.fields {
                effects.push(Effect::SaveSetting { key: field.key.clone(), value: field.value.clone() });
            }
            rebuild_settings_fields(state);
            effects
        }
        Action::SettingsDiscard => {
            state.tab_states.settings.editing_field = None;
            state.tab_states.settings.draft_value.clear();
            state.tab_states.settings.has_unsaved_changes = false;
            rebuild_settings_fields(state);
            vec![]
        }
        Action::RefreshDashboardStats => vec![Effect::RefreshDashboardStats],
        Action::Logout => vec![Effect::Logout],

        _ => vec![],
    }
}

fn reduce_modal(app: &mut App, action: Action) -> Vec<Effect> {
    let AppState::Running(state) = &mut app.state else { return vec![] };
    match action {
        Action::DismissModal | Action::ConfirmModal => {
            state.modal_stack.pop();
            vec![]
        }
        _ => vec![],
    }
}

fn reduce_command_palette(app: &mut App, action: Action) -> Vec<Effect> {
    let AppState::Running(state) = &mut app.state else { return vec![] };
    match action {
        Action::CommandPaletteDismiss => {
            state.command_palette = None;
            vec![]
        }
        Action::CommandPaletteFilter(s) => {
            if let Some(p) = state.command_palette.as_mut() {
                p.input = s;
            }
            vec![]
        }
        Action::CommandPaletteExecute(_) => {
            // Execute the selected command
            let selected_cmd = state
                .command_palette
                .as_ref()
                .and_then(|p| p.results.get(p.selected))
                .map(|c| c.action.clone());
            state.command_palette = None;
            if let Some(cmd) = selected_cmd {
                match cmd {
                    CommandAction::Quit => { app.should_quit = true; vec![] }
                    CommandAction::SwitchTab(t) => { state.active_tab = t; vec![] }
                    CommandAction::OpenHelp => {
                        state.modal_stack.push(Modal::Help(crate::modals::HelpModal { scroll: 0 }));
                        vec![]
                    }
                    CommandAction::ToggleCommandPalette => vec![], // already open
                    CommandAction::RefreshDashboard => vec![Effect::RefreshDashboardStats],
                    CommandAction::StartSync => {
                        // Trigger start sync
                        let effects = reduce_running(app, Action::DashboardStartSync);
                        effects
                    }
                    CommandAction::Logout => vec![Effect::Logout],
                }
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

fn tab_target_to_kind(t: TabTarget) -> TabKind {
    match t {
        TabTarget::Dashboard => TabKind::Dashboard,
        TabTarget::Explorer => TabKind::Explorer,
        TabTarget::SyncCenter => TabKind::SyncCenter,
        TabTarget::ActivityLog => TabKind::ActivityLog,
        TabTarget::Settings => TabKind::Settings,
    }
}

fn dirs_next_default() -> std::path::PathBuf {
    dirs::home_dir()
        .map(|h| h.join("git-vacuum"))
        .unwrap_or_else(|| std::path::PathBuf::from("./git-vacuum"))
}

pub fn reduce_event(app: &mut App, event: AppEvent) -> Vec<Effect> {
    match event {
        AppEvent::AuthSucceeded { info } => {
            // Transition to Running state
            let mut state = RunningAppState::new(info);
            state.loading.repos = true;
            state.loading.stats = true;
            app.state = AppState::Running(state);
            // Load any cached repos immediately, then kick off fresh discovery
            vec![
                Effect::LoadReposFromDb,
                Effect::DiscoverRepos { source: RepoSource::MyRepos },
                Effect::RefreshDashboardStats,
            ]
        }
        AppEvent::ReposLoaded { entries } => {
            if let AppState::Running(state) = &mut app.state {
                state.repos = entries;
                state.tab_states.explorer.cursor = 0;
                if let Some(welcome) = state.welcome_state.as_mut() {
                    welcome.repos_count = Some(state.repos.len());
                }
                state.loading.repos = false;
            }
            vec![]
        }
        AppEvent::AuthFailed { reason, detail } => {
            if let AppState::Auth(auth) = &mut app.state {
                auth.loading = false;
                auth.error = Some(classify_auth_error(&reason, &detail));
                auth.phase = AuthPhase::AuthFailed;
                auth.failed_focus = 0;
            }
            vec![]
        }
        AppEvent::ReposDiscovered { source: _, count } => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.explorer.loading = false;
                if let Some(welcome) = state.welcome_state.as_mut() {
                    if welcome.repos_count.is_none() {
                        welcome.repos_count = Some(count);
                    }
                }
            }
            vec![]
        }
        AppEvent::DiscoveryFailed { error } => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.explorer.loading = false;
                state.modal_stack.push(Modal::ErrorDetail(crate::modals::ErrorDetailModal {
                    repo_full_name: "Discovery".into(),
                    error_message: error,
                    raw_output: String::new(),
                }));
            }
            vec![]
        }
        AppEvent::SyncCloneStarted { job_id, repo_full_name } => {
            log_event(app, job_id, repo_full_name, LogStatus::Active, "cloning…");
            vec![]
        }
        AppEvent::SyncCloneProgress { job_id, repo_full_name, bytes, total } => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
                    entry.detail = format!("{} / {}", human_bytes(bytes), human_bytes(total));
                }
            }
            vec![]
        }
        AppEvent::SyncCloneCompleted { job_id, repo_full_name, size_bytes, .. } => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
                    entry.status = LogStatus::Success;
                    entry.detail = format!("cloned ({})", human_bytes(size_bytes));
                }
            }
            vec![]
        }
        AppEvent::SyncFetchStarted { job_id, repo_full_name } => {
            log_event(app, job_id, repo_full_name, LogStatus::Active, "fetching…");
            vec![]
        }
        AppEvent::SyncFetchProgress { job_id, repo_full_name: _, bytes } => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
                    entry.detail = format!("fetched {}", human_bytes(bytes));
                }
            }
            vec![]
        }
        AppEvent::SyncFetchCompleted { job_id, repo_full_name, new_commits, bytes_fetched, .. } => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
                    entry.status = LogStatus::Success;
                    entry.detail = format!("synced (+{} commits, {})", new_commits, human_bytes(bytes_fetched));
                }
            }
            vec![]
        }
        AppEvent::SyncRepoFailed { job_id, repo_full_name, error } => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
                    entry.status = LogStatus::Failed;
                    entry.detail = error;
                }
            }
            vec![]
        }
        AppEvent::SyncRepoUpToDate { job_id, repo_full_name } => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
                    entry.status = LogStatus::Success;
                    entry.detail = "up to date".into();
                }
            }
            vec![]
        }
        AppEvent::SyncAllCompleted { summary } => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.sync_center.phase = crate::tabs::SyncPhase::Completed(summary);
            }
            vec![]
        }
        AppEvent::SyncPaused => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.sync_center.phase = crate::tabs::SyncPhase::Paused;
            }
            vec![]
        }
        AppEvent::SyncResumed => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.sync_center.phase = crate::tabs::SyncPhase::Active;
            }
            vec![]
        }
        AppEvent::SyncCancelled { summary } => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.sync_center.phase = crate::tabs::SyncPhase::Cancelled(summary);
            }
            vec![]
        }
        AppEvent::StatsRefreshed => {
            // No-op: kept for backwards-compat; new flow is DashboardStatsUpdated
            vec![]
        }
        AppEvent::DashboardStatsUpdated { stats, attention } => {
            if let AppState::Running(state) = &mut app.state {
                state.tab_states.dashboard.stats = Some(stats);
                state.tab_states.dashboard.attention_list = attention;
                state.tab_states.dashboard.loading = false;
                state.loading.stats = false;
            }
            vec![]
        }
        AppEvent::Tick => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(welcome) = state.welcome_state.as_mut() {
                    let elapsed = welcome.entered_at.elapsed();
                    welcome.phase = if elapsed < std::time::Duration::from_millis(500) {
                        WelcomePhase::Greeting
                    } else if elapsed < std::time::Duration::from_millis(1500) {
                        WelcomePhase::Summary
                    } else {
                        WelcomePhase::Ready
                    };
                    // Auto-dismiss after 8 seconds regardless of phase
                    if elapsed > std::time::Duration::from_secs(8) {
                        state.welcome_state = None;
                    }
                }
            }
            vec![]
        }
        AppEvent::WelcomeAdvanced => {
            if let AppState::Running(state) = &mut app.state {
                if let Some(welcome) = state.welcome_state.as_mut() {
                    welcome.phase = match welcome.phase {
                        WelcomePhase::Greeting => WelcomePhase::Summary,
                        WelcomePhase::Summary => WelcomePhase::Ready,
                        WelcomePhase::Ready => {
                            state.welcome_state = None;
                            return vec![];
                        }
                    };
                }
            }
            vec![]
        }
        AppEvent::OAuthCodeReceived { user_code, verification_uri, expires_in } => {
            if let AppState::Auth(auth) = &mut app.state {
                auth.loading = false;
                auth.mode = AuthMode::OAuth;
                auth.phase = AuthPhase::DeviceActivation;
                auth.show_url_prompt = true;
                // The device_code is set in the binary's spawned task; we
                // populate what we can in the reducer.
                auth.oauth = Some(crate::state::OAuthState {
                    user_code,
                    verification_uri,
                    device_code: String::new(),
                    interval_secs: 5,
                    expires_at: std::time::Instant::now() + expires_in,
                    poll_attempt: 0,
                    last_poll: None,
                });
            }
            vec![]
        }
        AppEvent::OAuthTokenReceived { token, scopes: _ } => {
            // Token received from OAuth; hand it to the service to validate
            // and store in keyring. The service will then return UserInfo and
            // we transition to Running.
            if let AppState::Auth(auth) = &mut app.state {
                auth.phase = AuthPhase::Validating;
            }
            vec![Effect::CompleteOAuthWithToken { token }]
        }
        AppEvent::OAuthTimeout => {
            if let AppState::Auth(auth) = &mut app.state {
                auth.loading = false;
                auth.oauth = None;
                auth.phase = AuthPhase::AuthFailed;
                auth.failed_focus = 0;
                auth.error = Some(AuthErrorView {
                    category: AuthErrorCategory::Other,
                    headline: "OAuth code expired".into(),
                    detail: "The device code expired before you authorized. Try again.".into(),
                    hints: vec![
                        "Press Enter to go back to the device activation screen.".into(),
                    ],
                });
            }
            vec![]
        }
        AppEvent::LoggedOut => {
            let mut auth = AuthScreenState::default();
            auth.oauth_client_id = app.oauth_client_id.clone();
            app.state = AppState::Auth(auth);
            vec![]
        }
        AppEvent::SyncProgressUpdated { progress: _ } => vec![],
        AppEvent::FatalError { message } => {
            app.state = AppState::FatalError(message);
            vec![]
        }
    }
}

fn rebuild_settings_fields(state: &mut RunningAppState) {
    if state.tab_states.settings.editing_field.is_some() {
        return;
    }
    let cat = state.tab_states.settings.selected_category;
    let mut fields = Vec::new();
    match cat {
        SettingsCategory::General => {
            fields.push(git_vacuum_core::SettingsField {
                key: "clone_path".into(),
                label: "Clone path".into(),
                value: state.clone_path.clone(),
                kind: git_vacuum_core::SettingsFieldKind::Path,
                help: Some("Directory where repositories are cloned.".into()),
            });
            fields.push(git_vacuum_core::SettingsField {
                key: "concurrency".into(),
                label: "Concurrency".into(),
                value: state.tab_states.sync_center.concurrency.to_string(),
                kind: git_vacuum_core::SettingsFieldKind::Integer { min: 1, max: 64 },
                help: Some("Max parallel clone/fetch operations.".into()),
            });
        }
        SettingsCategory::Clone => {
            fields.push(git_vacuum_core::SettingsField {
                key: "skip_archived".into(),
                label: "Skip archived repos".into(),
                value: state.tab_states.explorer.skip_archived.to_string(),
                kind: git_vacuum_core::SettingsFieldKind::Boolean,
                help: Some("Hide archived repositories in the Explorer.".into()),
            });
            fields.push(git_vacuum_core::SettingsField {
                key: "skip_forks".into(),
                label: "Skip forks".into(),
                value: state.tab_states.explorer.skip_forks.to_string(),
                kind: git_vacuum_core::SettingsFieldKind::Boolean,
                help: Some("Hide forked repositories in the Explorer.".into()),
            });
        }
        SettingsCategory::Sync => {
            fields.push(git_vacuum_core::SettingsField {
                key: "concurrency".into(),
                label: "Concurrency".into(),
                value: state.tab_states.sync_center.concurrency.to_string(),
                kind: git_vacuum_core::SettingsFieldKind::Integer { min: 1, max: 64 },
                help: Some("Max parallel clone/fetch operations.".into()),
            });
            fields.push(git_vacuum_core::SettingsField {
                key: "retry_failed".into(),
                label: "Retry failed jobs".into(),
                value: "true".into(),
                kind: git_vacuum_core::SettingsFieldKind::Boolean,
                help: Some("Retry failed repositories automatically.".into()),
            });
        }
        SettingsCategory::GitHub => {
            fields.push(git_vacuum_core::SettingsField {
                key: "default_source".into(),
                label: "Default source".into(),
                value: format!("{:?}", state.tab_states.explorer.source),
                kind: git_vacuum_core::SettingsFieldKind::Dropdown {
                    options: vec!["MyRepos".into(), "Starred".into(), "All".into()],
                },
                help: Some("Which repository list to load on startup.".into()),
            });
            fields.push(git_vacuum_core::SettingsField {
                key: "org_input".into(),
                label: "Organization".into(),
                value: state.tab_states.explorer.org_input.clone(),
                kind: git_vacuum_core::SettingsFieldKind::Text,
                help: Some("Organization login when using Org source.".into()),
            });
            fields.push(git_vacuum_core::SettingsField {
                key: "topic_filter".into(),
                label: "Topic filter".into(),
                value: state.tab_states.explorer.topic_filter.clone(),
                kind: git_vacuum_core::SettingsFieldKind::Text,
                help: Some("Filter repositories by topic.".into()),
            });
        }
        SettingsCategory::Advanced => {
            fields.push(git_vacuum_core::SettingsField {
                key: "timeout_secs".into(),
                label: "Timeout per job (seconds)".into(),
                value: "1800".into(),
                kind: git_vacuum_core::SettingsFieldKind::Integer { min: 60, max: 86400 },
                help: Some("Max seconds allowed for one clone/fetch job.".into()),
            });
        }
    }
    state.tab_states.settings.fields = fields;
    if state.tab_states.settings.selected_field >= state.tab_states.settings.fields.len() && !state.tab_states.settings.fields.is_empty() {
        state.tab_states.settings.selected_field = 0;
    }
}

fn apply_setting_value(state: &mut RunningAppState, key: &str, value: &str) {
    match key {
        "clone_path" => state.clone_path = value.to_string(),
        "concurrency" => {
            if let Ok(n) = value.parse::<usize>() {
                state.tab_states.sync_center.concurrency = n.clamp(1, 64);
            }
        }
        "skip_archived" => state.tab_states.explorer.skip_archived = value == "true",
        "skip_forks" => state.tab_states.explorer.skip_forks = value == "true",
        "org_input" => state.tab_states.explorer.org_input = value.to_string(),
        "topic_filter" => state.tab_states.explorer.topic_filter = value.to_string(),
        "default_source" => {
            state.tab_states.explorer.source = match value {
                "Starred" => git_vacuum_core::RepoSource::Starred,
                "All" => git_vacuum_core::RepoSource::All,
                _ => git_vacuum_core::RepoSource::MyRepos,
            };
        }
        _ => {}
    }
}

fn log_event(app: &mut App, job_id: JobId, repo_full_name: String, status: LogStatus, detail: &str) {
    if let AppState::Running(state) = &mut app.state {
        if let Some(entry) = state.tab_states.sync_center.live_log.iter_mut().find(|e| e.job_id == job_id) {
            entry.status = status;
            entry.detail = detail.to_string();
        } else {
            state.tab_states.sync_center.live_log.push(crate::tabs::LogEntry {
                job_id,
                repo_full_name,
                status,
                detail: detail.to_string(),
            });
            // Trim to last 500
            let log_len = state.tab_states.sync_center.live_log.len();
            if log_len > 500 {
                let drop_n = log_len - 500;
                state.tab_states.sync_center.live_log.drain(0..drop_n);
            }
        }
    }
}

fn human_bytes(bytes: u64) -> String {
    git_vacuum_core::human_bytes(bytes)
}

/// Map a (reason, detail) pair from `AppEvent::AuthFailed` into a structured
/// `AuthErrorView` for the new auth screen. We inspect the strings (rather
/// than expanding the error types) because the reasons are emitted by the
/// binary crate's effect dispatch.
fn classify_auth_error(reason: &str, detail: &str) -> AuthErrorView {
    let detail_lower = detail.to_lowercase();

    let (category, headline) = match reason {
        "oauth_init_failed" | "oauth_validate_failed" => {
            (AuthErrorCategory::OAuthConfig, "OAuth setup failed".to_string())
        }
        "oauth_poll_failed" => {
            (AuthErrorCategory::OAuthConfig, "OAuth polling failed".to_string())
        }
        "access_denied" => {
            (AuthErrorCategory::AccessDenied, "Authorization denied".to_string())
        }
        _ => {
            if detail_lower.contains("scope") {
                (AuthErrorCategory::InsufficientScopes, "Token missing required scopes".to_string())
            } else if detail_lower.contains("expired") {
                (AuthErrorCategory::ExpiredToken, "Token expired".to_string())
            } else if detail_lower.contains("network") || detail_lower.contains("timeout") {
                (AuthErrorCategory::Network, "Network error".to_string())
            } else {
                (AuthErrorCategory::InvalidToken, "Invalid token".to_string())
            }
        }
    };

    let hints = match category {
        AuthErrorCategory::InvalidToken => vec![
            "Verify the token at https://github.com/settings/tokens".into(),
            "Make sure the token starts with ghp_ or github_pat_.".into(),
        ],
        AuthErrorCategory::ExpiredToken => vec![
            "Generate a new token at https://github.com/settings/tokens".into(),
        ],
        AuthErrorCategory::InsufficientScopes => vec![
            "Required scopes: repo, read:org, user".into(),
            "Re-generate the token with these scopes enabled.".into(),
        ],
        AuthErrorCategory::Network => vec![
            "Check your internet connection.".into(),
            "GitHub status: https://www.githubstatus.com".into(),
        ],
        AuthErrorCategory::OAuthConfig => vec![
            "Register an OAuth App at https://github.com/settings/applications/new".into(),
            "Set GIT_VACUUM_OAUTH_CLIENT_ID or pass --oauth-client-id <id>.".into(),
        ],
        AuthErrorCategory::AccessDenied => vec![
            "Re-run the flow and click 'Authorize' on the GitHub page.".into(),
        ],
        AuthErrorCategory::Other => Vec::new(),
    };

    AuthErrorView {
        category,
        headline,
        detail: detail.to_string(),
        hints,
    }
}

// Public re-exports for binary crate to call service functions
pub use git_vacuum_service as service;
pub async fn auth_pat(app: &App, token: String) -> Result<git_vacuum_core::UserInfo, git_vacuum_core::AuthError> {
    authenticate_pat(app.services.clone(), &token).await
}
pub async fn load_creds(app: &App) -> Result<Option<git_vacuum_core::UserInfo>, git_vacuum_core::AuthError> {
    load_stored_credentials(app.services.clone()).await
}
pub async fn do_logout(app: &App) -> Result<(), git_vacuum_core::KeyringError> {
    logout(app.services.clone()).await
}
pub async fn do_discover(app: &App, source: git_vacuum_core::RepoSource) -> Result<Vec<git_vacuum_core::RepoEntry>, git_vacuum_core::DiscoveryError> {
    run_discovery(app.services.clone(), source).await
}

// Unit tests for the reducer require stub trait implementations for all four
// infrastructure traits (Database, GithubApi, GitOps, KeyringStore). Building
// these stubs requires `async-trait` and a lot of boilerplate. The reducer is
// pure (no I/O) — its tests live better as part of the binary crate's
// integration tests once the main loop is in place. For now, the build
// validates that the reducer compiles; runtime behavior is verified in M4.
//
// TODO(M4): Add `tests/reducer.rs` in the binary crate with proper stubs.

#[cfg(test)]
mod tests {
#[test]
    fn placeholder_compiles() {
        // Real reducer tests live in the binary crate (see TODO in plan).
    }
}
