use std::path::PathBuf;

use git_vacuum_core::RepoSource;

#[derive(Debug, Clone, Default)]
pub struct TabStates {
    pub dashboard: DashboardTabState,
    pub explorer: ExplorerTabState,
    pub sync_center: SyncCenterTabState,
    pub activity_log: ActivityLogTabState,
    pub settings: SettingsTabState,
}

#[derive(Debug, Clone, Default)]
pub struct DashboardTabState {
    pub attention_list: Vec<git_vacuum_core::AttentionItem>,
    pub stats: Option<git_vacuum_core::DashboardStats>,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub struct ExplorerTabState {
    pub source: RepoSource,
    pub org_input: String,
    pub filter_text: String,
    pub topic_filter: String,
    pub skip_archived: bool,
    pub skip_forks: bool,
    pub mark_mode: bool,
    pub mark_start: Option<usize>,
    pub cursor: usize,
    pub table_scroll: usize,
    pub detail_scroll: usize,
    pub loading: bool,
}

impl Default for ExplorerTabState {
    fn default() -> Self {
        Self {
            source: RepoSource::MyRepos,
            org_input: String::new(),
            filter_text: String::new(),
            topic_filter: String::new(),
            skip_archived: false,
            skip_forks: false,
            mark_mode: false,
            mark_start: None,
            cursor: 0,
            table_scroll: 0,
            detail_scroll: 0,
            loading: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum SyncPhase {
    #[default]
    Idle,
    PreSync,
    Active,
    Paused,
    Completed(git_vacuum_core::SyncSummary),
    Cancelled(git_vacuum_core::PartialSyncSummary),
}

#[derive(Debug, Clone, Default)]
pub struct SyncCenterTabState {
    pub phase: SyncPhase,
    pub live_log: Vec<LogEntry>,
    pub log_follow: bool,
    pub log_scroll: usize,
    pub overall: Option<git_vacuum_core::OverallProgress>,
    pub concurrency: usize,
    pub queued_repos: usize,
    pub base_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub job_id: git_vacuum_core::JobId,
    pub repo_full_name: String,
    pub status: LogStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStatus {
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
    pub run_detail_scroll: usize,
    pub filter_text: String,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub struct SettingsTabState {
    pub fields: Vec<git_vacuum_core::SettingsField>,
    pub selected_field: usize,
    pub editing_field: Option<usize>,
    pub draft_value: String,
    pub has_unsaved_changes: bool,
    pub selected_category: git_vacuum_core::SettingsCategory,
}

impl Default for SettingsTabState {
    fn default() -> Self {
        Self {
            fields: Vec::new(),
            selected_field: 0,
            editing_field: None,
            draft_value: String::new(),
            has_unsaved_changes: false,
            selected_category: git_vacuum_core::SettingsCategory::General,
        }
    }
}
