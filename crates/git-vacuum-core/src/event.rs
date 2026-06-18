use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::types::activity::ExportFormat;
use crate::types::job::JobId;
use crate::types::progress::OverallProgress;
use crate::types::repo::{RepoEntry, RepoSource};
use crate::types::sync::{PartialSyncSummary, SyncSummary};
use crate::types::user::UserInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key(crossterm::event::KeyEvent),
    Resize(u16, u16),
    Tick,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Quit,
    OpenHelp,
    OpenCommandPalette,
    DismissModal,
    ConfirmModal,
    NoOp,

    SwitchTab(TabTarget),
    NextTab,
    PrevTab,
    SwitchTabByNumber(u8),
    DismissWelcome,

    AuthSubmitToken(String),
    AuthTokenInputChanged(String),
    AuthStartOAuth,
    AuthStartOAuthNow,
    AuthStartPAT,
    AuthCancelOAuth,
    AuthSkipForPublic,
    AuthMethodSelected(AuthMethodChoice),
    AuthBackToMethodPicker,
    AuthMethodCursorMoved(i8),
    AuthFailedFocusMoved(i8),
    AuthOpenOAuthUrl,
    AuthCopyOAuthCode,
    AuthDismissUrlPrompt,

    ExplorerToggle(usize),
    ExplorerSelectAll,
    ExplorerDeselectAll,
    ExplorerStartMarkMode,
    ExplorerMarkTo(usize),
    ExplorerEndMarkMode,
    ExplorerSetFilter(String),
    ExplorerClearFilter,
    ExplorerSetSortColumn(u8),
    ExplorerSetOrgInput(String),
    ExplorerSetTopicFilter(String),
    ExplorerToggleSkipArchived,
    ExplorerToggleSkipForks,
    ExplorerStartSync,
    ExplorerInspect(usize),
    ExplorerOpenBrowser(usize),
    ExplorerRefresh,

    SyncStart,
    SyncPause,
    SyncResume,
    SyncCancel,
    SyncShowErrorsOnly,
    SyncShowAll,
    SyncToggleFollow,
    SyncScrollUp,
    SyncScrollDown,
    SyncViewFailedDetails,

    DashboardRefreshStats,
    DashboardStartSync,
    DashboardInspect(usize),

    ActivityViewRun(usize),
    ActivityRetryRun(usize),
    ActivityExportRun(usize),
    ActivitySetFilter(String),

    SettingsNavigate(usize),
    SettingsEdit(usize),
    SettingsToggle(usize),
    SettingsSelectDropdown(usize),
    SettingsDropdownPick(usize),
    SettingsSave,
    SettingsDiscard,
    SettingsSwitchCategory(usize),

    CommandPaletteFilter(String),
    CommandPaletteExecute(String),
    CommandPaletteDismiss,

    RefreshDashboardStats,
    LoadStoredCredentials,
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabTarget {
    Dashboard,
    Explorer,
    SyncCenter,
    ActivityLog,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AuthMethodChoice {
    #[default]
    Pat,
    OAuth,
    GhCli,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    AuthSucceeded {
        info: UserInfo,
    },
    AuthFailed {
        reason: String,
        detail: String,
    },
    OAuthCodeReceived {
        user_code: String,
        verification_uri: String,
        expires_in: Duration,
    },
    OAuthTokenReceived {
        token: String,
        scopes: Vec<String>,
    },
    OAuthTimeout,
    LoggedOut,

    ReposDiscovered {
        source: RepoSource,
        count: usize,
    },
    DiscoveryFailed {
        error: String,
    },

    SyncCloneStarted {
        job_id: JobId,
        repo_full_name: String,
    },
    SyncCloneProgress {
        job_id: JobId,
        repo_full_name: String,
        bytes: u64,
        total: u64,
    },
    SyncCloneCompleted {
        job_id: JobId,
        repo_full_name: String,
        size_bytes: u64,
        duration: Duration,
    },
    SyncFetchStarted {
        job_id: JobId,
        repo_full_name: String,
    },
    SyncFetchProgress {
        job_id: JobId,
        repo_full_name: String,
        bytes: u64,
    },
    SyncFetchCompleted {
        job_id: JobId,
        repo_full_name: String,
        new_commits: u32,
        bytes_fetched: u64,
        duration: Duration,
    },
    SyncRepoFailed {
        job_id: JobId,
        repo_full_name: String,
        error: String,
    },
    SyncRepoUpToDate {
        job_id: JobId,
        repo_full_name: String,
    },

    SyncAllCompleted {
        summary: SyncSummary,
    },
    SyncPaused,
    SyncResumed,
    SyncCancelled {
        summary: PartialSyncSummary,
    },
    SyncProgressUpdated {
        progress: OverallProgress,
    },

    StatsRefreshed,
    ReposLoaded {
        entries: Vec<RepoEntry>,
    },
    DashboardStatsUpdated {
        stats: crate::DashboardStats,
        attention: Vec<crate::AttentionItem>,
    },
    Tick,
    WelcomeAdvanced,
    FatalError {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum Effect {
    AuthenticatePat {
        token: String,
    },
    StartOAuthDeviceFlow {
        client_id: String,
        scopes: Vec<String>,
    },
    PollOAuthToken {
        client_id: String,
        device_code: String,
        interval: Duration,
    },
    CancelOAuth,
    CompleteOAuthWithToken {
        token: String,
    },
    LoadStoredCredentials,
    Logout,

    DiscoverRepos {
        source: RepoSource,
    },
    PersistRepoSelection {
        github_ids: Vec<i64>,
        selected: bool,
    },
    LoadReposFromDb,

    StartSync {
        repos: Vec<RepoEntry>,
        base_path: PathBuf,
        concurrency: usize,
    },
    CloneSingle {
        repo: RepoEntry,
        base_path: PathBuf,
    },
    SyncSingle {
        repo: RepoEntry,
        local_path: PathBuf,
    },
    PauseSync,
    ResumeSync,
    CancelSync,

    RefreshDashboardStats,

    RecordSyncRun {
        run_id: Option<i64>,
        summary: SyncSummary,
        options_json: Option<String>,
    },
    ExportRun {
        run_id: i64,
        format: ExportFormat,
        path: PathBuf,
    },

    SaveSetting {
        key: String,
        value: String,
    },
    TestConnection,

    PersistRepos {
        entries: Vec<RepoEntry>,
    },
    MarkReposDeleted {
        github_ids: Vec<i64>,
    },

    OpenUrl {
        url: String,
    },
    CopyToClipboard {
        text: String,
    },

    None,
}

#[derive(Debug, Clone)]
pub struct EventBus {
    pub app_tx: mpsc::UnboundedSender<AppEvent>,
    pub progress_tx: mpsc::UnboundedSender<AppEvent>,
    pub cancel_tx: watch::Sender<bool>,
}

impl EventBus {
    pub fn new() -> (Self, EventBusHandle) {
        let (app_tx, app_rx) = mpsc::unbounded_channel();
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        let handle = EventBusHandle {
            app_rx,
            progress_rx,
            cancel_rx,
        };
        let bus = Self {
            app_tx,
            progress_tx,
            cancel_tx,
        };
        (bus, handle)
    }
}

#[derive(Debug)]
pub struct EventBusHandle {
    pub app_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub progress_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub cancel_rx: watch::Receiver<bool>,
}
