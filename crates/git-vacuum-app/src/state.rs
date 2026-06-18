use std::time::Instant;

use git_vacuum_core::UserInfo;
pub use git_vacuum_core::AuthMethodChoice;
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
    pub phase: AuthPhase,
    pub method: AuthMethodChoice,
    pub method_cursor: u8,
    pub last_method: AuthMethodChoice,
    pub token_input: String,
    pub error: Option<AuthErrorView>,
    pub loading: bool,
    pub mode: AuthMode,
    pub oauth: Option<OAuthState>,
    /// True when the OAuth code has just arrived and we should prompt to
    /// open the verification URL in the browser. Cleared by Enter/Esc.
    pub show_url_prompt: bool,
    /// 0 = "Try Again" (default), 1 = "Pick a different method". Used
    /// only on the AuthFailed screen.
    pub failed_focus: u8,
    /// OAuth App client_id (set on app construction; required for OAuth flow).
    pub oauth_client_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthPhase {
    #[default]
    MethodPicker,
    PatInput,
    Validating,
    DeviceActivation,
    AuthFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthErrorCategory {
    InvalidToken,
    ExpiredToken,
    InsufficientScopes,
    Network,
    OAuthConfig,
    AccessDenied,
    Other,
}

#[derive(Debug, Clone)]
pub struct AuthErrorView {
    pub category: AuthErrorCategory,
    pub headline: String,
    pub detail: String,
    pub hints: Vec<String>,
}

impl AuthErrorView {
    pub fn simple(detail: impl Into<String>) -> Self {
        Self {
            category: AuthErrorCategory::Other,
            headline: "Authentication failed".into(),
            detail: detail.into(),
            hints: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OAuthState {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval_secs: u64,
    pub expires_at: Instant,
    pub poll_attempt: u32,
    pub last_poll: Option<Instant>,
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
    pub loading: LoadingState,
    pub welcome_state: Option<WelcomeState>,
}

#[derive(Debug, Clone, Default)]
pub struct LoadingState {
    pub repos: bool,
    pub stats: bool,
}

impl LoadingState {
    pub fn anything_pending(&self) -> bool {
        self.repos || self.stats
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomePhase {
    Greeting,
    Summary,
    Ready,
}

#[derive(Debug, Clone)]
pub struct WelcomeState {
    pub user: UserInfo,
    pub entered_at: Instant,
    pub repos_count: Option<usize>,
    pub phase: WelcomePhase,
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
            authenticated_user: Some(user.clone()),
            clone_path: String::new(),
            loading: LoadingState::default(),
            welcome_state: Some(WelcomeState {
                user,
                entered_at: Instant::now(),
                repos_count: None,
                phase: WelcomePhase::Greeting,
            }),
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
