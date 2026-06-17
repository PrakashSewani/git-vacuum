use git_vacuum_core::JobId;

#[derive(Debug, Clone)]
pub enum Modal {
    Confirmation(ConfirmationModal),
    RepoDetail(RepoDetailModal),
    ErrorDetail(ErrorDetailModal),
    Help(HelpModal),
    InputPrompt(InputPromptModal),
}

#[derive(Debug, Clone)]
pub struct ConfirmationModal {
    pub title: String,
    pub message: String,
    pub items: Vec<String>,
    pub confirm_label: String,
    pub cancel_label: String,
    pub focus: ModalFocus,
    pub danger: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalFocus {
    Confirm,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct RepoDetailModal {
    pub full_name: String,
    pub scroll: usize,
}

#[derive(Debug, Clone)]
pub struct ErrorDetailModal {
    pub repo_full_name: String,
    pub error_message: String,
    pub raw_output: String,
}

#[derive(Debug, Clone)]
pub struct HelpModal {
    pub scroll: usize,
}

#[derive(Debug, Clone)]
pub struct InputPromptModal {
    pub title: String,
    pub prompt: String,
    pub value: String,
    pub mask_input: bool,
    pub cursor_pos: usize,
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub input: String,
    pub cursor_pos: usize,
    pub results: Vec<CommandEntry>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub action: CommandAction,
}

#[derive(Debug, Clone)]
pub enum CommandAction {
    Quit,
    SwitchTab(crate::state::TabKind),
    OpenHelp,
    ToggleCommandPalette,
    RefreshDashboard,
    StartSync,
    Logout,
}

impl CommandPaletteState {
    pub fn open() -> Self {
        Self {
            input: String::new(),
            cursor_pos: 0,
            results: default_commands(),
            selected: 0,
        }
    }
}

pub fn default_commands() -> Vec<CommandEntry> {
    use crate::state::TabKind;
    vec![
        CommandEntry { name: "Go to Dashboard".into(), description: "Switch to the Dashboard tab".into(), action: CommandAction::SwitchTab(TabKind::Dashboard) },
        CommandEntry { name: "Go to Explorer".into(), description: "Switch to the Explorer tab".into(), action: CommandAction::SwitchTab(TabKind::Explorer) },
        CommandEntry { name: "Go to Sync Center".into(), description: "Switch to the Sync Center tab".into(), action: CommandAction::SwitchTab(TabKind::SyncCenter) },
        CommandEntry { name: "Go to Activity Log".into(), description: "Switch to the Activity Log tab".into(), action: CommandAction::SwitchTab(TabKind::ActivityLog) },
        CommandEntry { name: "Go to Settings".into(), description: "Switch to the Settings tab".into(), action: CommandAction::SwitchTab(TabKind::Settings) },
        CommandEntry { name: "Refresh Dashboard Stats".into(), description: "Reload dashboard stats from DB".into(), action: CommandAction::RefreshDashboard },
        CommandEntry { name: "Start Sync".into(), description: "Start syncing selected repos".into(), action: CommandAction::StartSync },
        CommandEntry { name: "Show Help".into(), description: "Open the keyboard help overlay".into(), action: CommandAction::OpenHelp },
        CommandEntry { name: "Log out".into(), description: "Clear stored token and return to auth".into(), action: CommandAction::Logout },
        CommandEntry { name: "Quit".into(), description: "Exit git-vacuum".into(), action: CommandAction::Quit },
    ]
}

#[allow(dead_code)]
pub fn job_id_zero() -> JobId {
    JobId(0)
}
