use git_vacuum_core::UserInfo;
use crate::modals;
use crate::tabs::TabStates;

#[derive(Debug, Clone)]
pub enum AppState {
    Auth(AuthScreenState),
    Running(RunningAppState),
    FatalError(String),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Auth(AuthScreenState::default())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuthScreenState {
    pub token_input: String,
    pub error: Option<String>,
    pub loading: bool,
    pub mode: AuthMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    #[default]
    Pat,
    OAuth,
}

#[derive(Debug, Clone)]
pub struct RunningAppState {
    pub active_tab: TabKind,
    pub tab_states: TabStates,
    pub modal_stack: Vec<modals::Modal>,
    pub command_palette: Option<modals::CommandPaletteState>,
    pub repos: Vec<git_vacuum_core::RepoEntry>,
    pub selected_indices: Vec<usize>,
    pub authenticated_user: Option<UserInfo>,
    pub clone_path: String,
}

impl RunningAppState {
    pub fn new(user: UserInfo) -> Self {
        Self {
            active_tab: TabKind::Dashboard,
            tab_states: TabStates::default(),
            modal_stack: Vec::new(),
            command_palette: None,
            repos: Vec::new(),
            selected_indices: Vec::new(),
            authenticated_user: Some(user),
            clone_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Dashboard,
    Explorer,
    SyncCenter,
    ActivityLog,
    Settings,
}

impl TabKind {
    pub fn label(&self) -> &'static str {
        match self {
            TabKind::Dashboard => "Dashboard",
            TabKind::Explorer => "Explorer",
            TabKind::SyncCenter => "Sync Center",
            TabKind::ActivityLog => "Activity Log",
            TabKind::Settings => "Settings",
        }
    }

    pub fn all() -> [TabKind; 5] {
        [
            TabKind::Dashboard,
            TabKind::Explorer,
            TabKind::SyncCenter,
            TabKind::ActivityLog,
            TabKind::Settings,
        ]
    }

    pub fn next(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|t| t == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    pub fn prev(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|t| t == self).unwrap_or(0);
        all[(idx + all.len() - 1) % all.len()]
    }
}
