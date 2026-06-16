use std::path::PathBuf;

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::types::job::{JobId, ProgressSample};
use crate::types::org::OrgInfo;
use crate::types::repo::RepoEntry;
use crate::types::repo_source::RepoSource;
use crate::types::sync::{PartialSyncSummary, SyncOptions, SyncSummary};

use crate::traits::git_ops::FetchResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard = 0,
    Explorer = 1,
    SyncCenter = 2,
    ActivityLog = 3,
    Settings = 4,
}

#[derive(Debug, Clone)]
pub enum Action {
    SwitchTab(Tab),
    NextTab,
    PrevTab,
    Quit,
    OpenHelp,
    OpenCommandPalette,
    DismissModal,
    ConfirmModal,
    NoOp,

    ExplorerToggle(usize),
    ExplorerSelectAll,
    ExplorerDeselectAll,
    ExplorerStartMarkMode,
    ExplorerMarkTo(usize),
    ExplorerEndMarkMode,
    ExplorerSetFilter(String),
    ExplorerClearFilter,
    ExplorerSortColumn(u8),
    ExplorerStartSync,
    ExplorerInspect(usize),
    ExplorerOpenBrowser(usize),
    ExplorerSetSource(RepoSource),
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
    SettingsSave,
    SettingsDiscard,
    SettingsSwitchCategory(usize),

    AuthSubmitToken(String),
    AuthAppendToToken(String),    // append single character to token input
    AuthSetToken(String),         // replace token input with pasted text
    AuthBackspace,                 // delete last character
    AuthStartOAuth,
    AuthStartPAT,
    AuthCancelOAuth,
    AuthSkipForPublic,

    CommandPaletteFilter(String),
    CommandPaletteExecute(String),
    CommandPaletteDismiss,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    AuthSucceeded {
        username: String,
        scopes: Vec<String>,
        token_expires: Option<chrono::DateTime<chrono::Utc>>,
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
    },
    OAuthTimeout,

    ReposDiscovered {
        repos: Vec<RepoEntry>,
        source: RepoSource,
    },
    DiscoveryProgress {
        repos_found: usize,
        estimated_total: Option<usize>,
    },
    DiscoveryFailed {
        error: String,
    },

    SyncCloneStarted {
        repo_full_name: String,
        job_id: JobId,
    },
    SyncCloneProgress(ProgressSample),
    SyncCloneCompleted {
        repo_full_name: String,
        job_id: JobId,
        size_bytes: u64,
        duration: Duration,
    },
    SyncFetchStarted {
        repo_full_name: String,
        job_id: JobId,
    },
    SyncFetchCompleted {
        repo_full_name: String,
        job_id: JobId,
        result: FetchResult,
    },
    SyncRepoFailed {
        repo_full_name: String,
        job_id: JobId,
        error: String,
    },
    SyncRepoUpToDate {
        repo_full_name: String,
        job_id: JobId,
    },
    SyncRepoRetrying {
        repo_full_name: String,
        job_id: JobId,
        attempt: u32,
        delay: Duration,
        reason: String,
    },

    SyncAllStarted {
        run_id: i64,
        total_jobs: usize,
    },
    SyncAllCompleted {
        summary: SyncSummary,
    },
    SyncCancelled {
        summary: PartialSyncSummary,
    },
    SyncPaused,
    SyncResumed,
    SyncRateLimited {
        retry_in: Duration,
    },

    StatsRefreshed {
        total_repos: usize,
        up_to_date: usize,
        behind: usize,
        errors: usize,
        total_size_bytes: u64,
        attention_list: Vec<RepoEntry>,
    },

    OrgsDiscovered {
        orgs: Vec<OrgInfo>,
    },

    FatalError {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum Effect {
    AuthenticatePat { token: String },
    StartOAuthDeviceFlow { client_id: String, scopes: Vec<String> },
    PollOAuthToken { client_id: String, device_code: String, interval: Duration },
    LoadStoredCredentials,
    DiscoverRepos { source: RepoSource },
    StartSync { repos: Vec<RepoEntry>, options: SyncOptions, base_path: PathBuf },
    RefreshDashboardStats,
    RecordSyncRun { summary: SyncSummary },
    PersistSettings { settings: crate::types::settings::AppSettings },
    SaveSettings { settings: Vec<(String, String)> },
    TestConnection,
    None,
}

pub struct EventBus {
    pub app_tx: mpsc::UnboundedSender<AppEvent>,
    pub app_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub progress_tx: mpsc::UnboundedSender<AppEvent>,
    pub progress_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub cancel_tx: watch::Sender<bool>,
    pub cancel_rx: watch::Receiver<bool>,
}

impl EventBus {
    pub fn new() -> Self {
        let (app_tx, app_rx) = mpsc::unbounded_channel();
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        Self { app_tx, app_rx, progress_tx, progress_rx, cancel_tx, cancel_rx }
    }
}
