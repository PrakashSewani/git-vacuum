

use git_vacuum_core::{Action, AppEvent, Effect, EventBus, Tab};

use git_vacuum_core::types::repo::RepoEntry;
use git_vacuum_core::types::repo_source::RepoSource;
use git_vacuum_core::types::sync::SyncOptions;


pub struct App {
    pub state: AppState,
    pub should_quit: bool,
    pub tick_count: u64,
    pub config: AppConfig,
    pub event_bus: EventBus,
}

pub struct AppConfig {
    pub clone_path: String,
    pub default_concurrency: usize,
    pub github_base_url: Option<String>,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub enum AppState {
    Auth(AuthScreenState),
    Running(RunningAppState),
    FatalError { message: String },
}

#[derive(Debug, Clone, Default)]
pub struct AuthScreenState {
    pub method: AuthMethod,
    pub token_input: String,
    pub oauth_user_code: Option<String>,
    pub oauth_verification_uri: Option<String>,
    pub oauth_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub oauth_polling: bool,
    pub error: Option<(String, String)>,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub enum AuthMethod {
    #[default]
    Pat,
    OAuthDevice,
    GhCli,
}

#[derive(Debug, Clone)]
pub struct RunningAppState {
    pub active_tab: Tab,
    pub tabs: TabStates,
    pub modal_stack: Vec<Modal>,
    pub command_palette: Option<CommandPaletteState>,
    pub username: String,
    pub repos: Vec<RepoEntry>,
    pub selected_indices: Vec<usize>,
    pub token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TabStates {
    pub dashboard: DashboardTabState,
    pub explorer: ExplorerTabState,
    pub sync_center: SyncCenterTabState,
    pub activity_log: ActivityLogTabState,
    pub settings: SettingsTabState,
}

#[derive(Debug, Clone, Default)]
pub struct DashboardTabState {
    pub total_repos: usize,
    pub up_to_date: usize,
    pub behind: usize,
    pub errors: usize,
    pub total_size_bytes: u64,
    pub attention_list: Vec<RepoEntry>,
    pub stats_loading: bool,
    pub scroll_offset: usize,
}

#[derive(Debug, Clone)]
pub struct ExplorerTabState {
    pub source: RepoSource,
    pub org_input: String,
    pub filter_text: String,
    pub skip_archived: bool,
    pub skip_forks: bool,
    pub sort_column: u8,
    pub sort_ascending: bool,
    pub mark_mode: bool,
    pub mark_start: Option<usize>,
    pub table_scroll: usize,
    pub loading: bool,
}

impl Default for ExplorerTabState {
    fn default() -> Self {
        Self {
            source: RepoSource::MyRepos,
            org_input: String::new(),
            filter_text: String::new(),
            skip_archived: true,
            skip_forks: true,
            sort_column: 2,
            sort_ascending: true,
            mark_mode: false,
            mark_start: None,
            table_scroll: 0,
            loading: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncCenterTabState {
    pub phase: SyncPhase,
    pub live_log: Vec<LogEntry>,
    pub log_filter: LogFilter,
    pub log_follow: bool,
    pub log_scroll: usize,
    pub progress_percent: f32,
    pub progress_done: usize,
    pub progress_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub elapsed_secs: u64,
    pub throughput_bytes_per_sec: f64,
}

impl Default for SyncCenterTabState {
    fn default() -> Self {
        Self {
            phase: SyncPhase::Idle,
            live_log: Vec::new(),
            log_filter: LogFilter::All,
            log_follow: true,
            log_scroll: 0,
            progress_percent: 0.0,
            progress_done: 0,
            progress_total: 0,
            bytes_done: 0,
            bytes_total: 0,
            elapsed_secs: 0,
            throughput_bytes_per_sec: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SyncPhase {
    Idle,
    PreSync { clone_count: usize, sync_count: usize },
    Active,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogFilter {
    All,
    ErrorsOnly,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub repo_full_name: String,
    pub status: LogEntryStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogEntryStatus {
    Queued,
    Active,
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Default)]
pub struct ActivityLogTabState {
    pub runs: Vec<git_vacuum_core::SyncRunRow>,
    pub selected_run: Option<usize>,
    pub filter_text: String,
    pub loading: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SettingsTabState {
    pub category: usize,
    pub editing_field: Option<(usize, String)>,
    pub has_unsaved_changes: bool,
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub input: String,
    pub matches: Vec<PaletteMatch>,
    pub history: Vec<String>,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct PaletteMatch {
    pub command: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum Modal {
    Confirmation {
        title: String,
        message: String,
        confirm_label: String,
        cancel_label: String,
        focus: ModalFocus,
        danger: bool,
    },
    RepoDetail {
        repo_index: usize,
        scroll: usize,
    },
    ErrorDetail {
        repo_full_name: String,
        error_message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    Help {
        scroll: usize,
    },
}

#[derive(Debug, Clone)]
pub enum ModalFocus {
    Cancel,
    Confirm,
}

impl App {
    pub fn new(config: AppConfig, event_bus: EventBus) -> Self {
        Self {
            state: AppState::Auth(AuthScreenState::default()),
            should_quit: false,
            tick_count: 0,
            config,
            event_bus,
        }
    }

    pub fn reduce(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Quit => {
                self.should_quit = true;
                vec![]
            }
            Action::SwitchTab(tab) => {
                if let AppState::Running(ref mut state) = self.state {
                    state.active_tab = tab;
                }
                vec![]
            }
            Action::NextTab => {
                if let AppState::Running(ref mut state) = self.state {
                    let idx = state.active_tab as usize;
                    state.active_tab = match idx {
                        4 => Tab::Dashboard,
                        n => unsafe { std::mem::transmute((n + 1) as u8) },
                    };
                }
                vec![]
            }
            Action::PrevTab => {
                if let AppState::Running(ref mut state) = self.state {
                    let idx = state.active_tab as usize;
                    state.active_tab = match idx {
                        0 => Tab::Settings,
                        n => unsafe { std::mem::transmute((n - 1) as u8) },
                    };
                }
                vec![]
            }
            Action::DismissModal => {
                if let AppState::Running(ref mut state) = self.state {
                    state.modal_stack.pop();
                }
                vec![]
            }
            Action::OpenHelp => {
                if let AppState::Running(ref mut state) = self.state {
                    state.modal_stack.push(Modal::Help { scroll: 0 });
                }
                vec![]
            }
            Action::AuthSubmitToken(token) => {
                if let AppState::Auth(ref mut state) = self.state {
                    state.status_message = Some("Verifying...".to_string());
                    state.error = None;
                }
                vec![Effect::AuthenticatePat { token }]
            }
            Action::AuthAppendToToken(text) => {
                if let AppState::Auth(ref mut state) = self.state {
                    let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
                    state.token_input.push_str(&cleaned);
                }
                vec![]
            }
            Action::AuthSetToken(text) => {
                if let AppState::Auth(ref mut state) = self.state {
                    let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
                    state.token_input = cleaned;
                }
                vec![]
            }
            Action::AuthBackspace => {
                if let AppState::Auth(ref mut state) = self.state {
                    state.token_input.pop();
                }
                vec![]
            }
            Action::AuthStartOAuth => {
                vec![Effect::StartOAuthDeviceFlow {
                    client_id: String::new(),
                    scopes: vec!["repo".to_string(), "read:org".to_string()],
                }]
            }
            Action::ExplorerRefresh => {
                if let AppState::Running(ref state) = self.state {
                    vec![Effect::DiscoverRepos {
                        source: state.tabs.explorer.source.clone(),
                    }]
                } else {
                    vec![]
                }
            }
            Action::ExplorerStartSync => {
                if let AppState::Running(ref mut state) = self.state {
                    let selected: Vec<RepoEntry> = state.selected_indices
                        .iter()
                        .filter_map(|&i| state.repos.get(i).cloned())
                        .collect();

                    let clone_count = selected.iter().filter(|r| r.clone_status == git_vacuum_core::CloneStatus::NotCloned).count();
                    let sync_count = selected.len() - clone_count;

                    state.tabs.sync_center.phase = SyncPhase::PreSync { clone_count, sync_count };
                    state.active_tab = Tab::SyncCenter;

                    vec![Effect::StartSync {
                        repos: selected,
                        options: SyncOptions::default(),
                        base_path: std::path::PathBuf::from(&self.config.clone_path),
                    }]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    pub fn reduce_event(&mut self, event: AppEvent) -> Vec<Effect> {
        match event {
            AppEvent::AuthSucceeded { username, scopes, token_expires } => {
                self.state = AppState::Running(RunningAppState {
                    active_tab: Tab::Dashboard,
                    tabs: TabStates {
                        dashboard: DashboardTabState::default(),
                        explorer: ExplorerTabState::default(),
                        sync_center: SyncCenterTabState::default(),
                        activity_log: ActivityLogTabState::default(),
                        settings: SettingsTabState::default(),
                    },
                    modal_stack: Vec::new(),
                    command_palette: None,
                    username,
                    repos: Vec::new(),
                    selected_indices: Vec::new(),
                    token: None,
                });
                vec![Effect::DiscoverRepos { source: RepoSource::AllAccessible }]
            }
            AppEvent::ReposDiscovered { repos, .. } => {
                if let AppState::Running(ref mut state) = self.state {
                    state.repos = repos;
                }
                vec![]
            }
            AppEvent::SyncAllStarted { run_id, total_jobs } => {
                if let AppState::Running(ref mut state) = self.state {
                    state.tabs.sync_center.phase = SyncPhase::Active;
                    state.tabs.sync_center.progress_total = total_jobs;
                    state.tabs.sync_center.progress_done = 0;
                    state.tabs.sync_center.live_log.clear();
                }
                vec![]
            }
            AppEvent::SyncCloneStarted { repo_full_name, .. } => {
                if let AppState::Running(ref mut state) = self.state {
                    state.tabs.sync_center.live_log.push(LogEntry {
                        repo_full_name,
                        status: LogEntryStatus::Active,
                        detail: "cloning...".to_string(),
                    });
                }
                vec![]
            }
            AppEvent::SyncCloneCompleted { repo_full_name, size_bytes, .. } => {
                if let AppState::Running(ref mut state) = self.state {
                    if let Some(entry) = state.tabs.sync_center.live_log
                        .iter_mut()
                        .find(|e| e.repo_full_name == repo_full_name)
                    {
                        entry.status = LogEntryStatus::Success;
                        entry.detail = format!("cloned ({})", git_vacuum_core::human_bytes(size_bytes));
                    }
                    state.tabs.sync_center.progress_done += 1;
                    state.tabs.sync_center.bytes_done += size_bytes;
                }
                vec![]
            }
            AppEvent::SyncAllCompleted { summary } => {
                if let AppState::Running(ref mut state) = self.state {
                    state.tabs.sync_center.phase = SyncPhase::Completed;
                }
                vec![]
            }
            AppEvent::SyncPaused => {
                if let AppState::Running(ref mut state) = self.state {
                    state.tabs.sync_center.phase = SyncPhase::Paused;
                }
                vec![]
            }
            AppEvent::SyncResumed => {
                if let AppState::Running(ref mut state) = self.state {
                    state.tabs.sync_center.phase = SyncPhase::Active;
                }
                vec![]
            }
            AppEvent::AuthFailed { reason, detail } => {
                if let AppState::Auth(ref mut state) = self.state {
                    state.error = Some((reason, detail));
                    state.status_message = None;
                }
                vec![]
            }
            _ => vec![],
        }
    }
}

