use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use git_vacuum_core::Action;

use git_vacuum_app::App;
use git_vacuum_app::{AppState, SyncPhase};

pub fn map_key_to_action(key: KeyEvent, app: &App) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Action::Quit,
            KeyCode::Char('a') => Action::ExplorerSelectAll,
            KeyCode::Char('d') => Action::ExplorerDeselectAll,
            _ => Action::NoOp,
        };
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::OpenHelp,
        KeyCode::Tab => Action::NextTab,
        KeyCode::Esc => {
            if matches!(app.state, AppState::Running(ref s) if !s.modal_stack.is_empty()) {
                Action::DismissModal
            } else {
                Action::NoOp
            }
        }

        KeyCode::Char('1') => Action::SwitchTab(git_vacuum_core::Tab::Dashboard),
        KeyCode::Char('2') => Action::SwitchTab(git_vacuum_core::Tab::Explorer),
        KeyCode::Char('3') => Action::SwitchTab(git_vacuum_core::Tab::SyncCenter),
        KeyCode::Char('4') => Action::SwitchTab(git_vacuum_core::Tab::ActivityLog),
        KeyCode::Char('5') => Action::SwitchTab(git_vacuum_core::Tab::Settings),

        KeyCode::Char(' ') | KeyCode::Char('t') => {
            match &app.state {
                AppState::Running(state) => {
                    match state.active_tab {
                        git_vacuum_core::Tab::Explorer => Action::ExplorerToggle(0),
                        _ => Action::NoOp,
                    }
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Char('r') => {
            match &app.state {
                AppState::Running(state) => {
                    match state.active_tab {
                        git_vacuum_core::Tab::Explorer => Action::ExplorerRefresh,
                        git_vacuum_core::Tab::Dashboard => Action::DashboardRefreshStats,
                        git_vacuum_core::Tab::SyncCenter => {
                            if matches!(state.tabs.sync_center.phase, SyncPhase::Paused) {
                                Action::SyncResume
                            } else {
                                Action::NoOp
                            }
                        }
                        _ => Action::NoOp,
                    }
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Char('s') => {
            match &app.state {
                AppState::Running(state) => {
                    match state.active_tab {
                        git_vacuum_core::Tab::Explorer => Action::ExplorerStartSync,
                        git_vacuum_core::Tab::Dashboard => Action::DashboardStartSync,
                        _ => Action::NoOp,
                    }
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Char('p') => {
            match &app.state {
                AppState::Running(state) => {
                    match state.active_tab {
                        git_vacuum_core::Tab::SyncCenter => Action::SyncPause,
                        _ => Action::NoOp,
                    }
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Char('c') => {
            match &app.state {
                AppState::Running(state) => {
                    match state.active_tab {
                        git_vacuum_core::Tab::SyncCenter => Action::SyncCancel,
                        _ => Action::NoOp,
                    }
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Enter => {
            match &app.state {
                AppState::Running(state) => {
                    match state.active_tab {
                        git_vacuum_core::Tab::SyncCenter => {
                            if matches!(state.tabs.sync_center.phase, SyncPhase::PreSync { .. }) {
                                Action::SyncStart
                            } else {
                                Action::NoOp
                            }
                        }
                        _ => Action::NoOp,
                    }
                }
                AppState::Auth(state) => {
                    if !state.token_input.is_empty() {
                        Action::AuthSubmitToken(state.token_input.clone())
                    } else {
                        Action::NoOp
                    }
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Char(c) => {
            match &app.state {
                AppState::Auth(_state) => {
                    Action::AuthAppendToToken(c.to_string())
                }
                _ => Action::NoOp,
            }
        }

        KeyCode::Backspace => {
            match &app.state {
                AppState::Auth(state) => {
                    if !state.token_input.is_empty() {
                        Action::AuthBackspace
                    } else {
                        Action::NoOp
                    }
                }
                _ => Action::NoOp,
            }
        }

        _ => Action::NoOp,
    }
}
