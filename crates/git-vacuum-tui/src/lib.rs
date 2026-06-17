pub mod components;
pub mod layout;
pub mod screens;
pub mod terminal;
pub mod theme;

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::state::AppState;
use git_vacuum_app::App;
use crate::components::{key_bar, tab_bar, title_bar};
use crate::layout::shell_layout;
use crate::screens::activity_log::render_activity_log;
use crate::screens::auth::render_auth;
use crate::screens::dashboard::render_dashboard;
use crate::screens::explorer::render_explorer;
use crate::screens::settings::render_settings;
use crate::screens::sync_center::render_sync_center;
use crate::theme::{COLOR_MUTED, COLOR_PRIMARY};

/// Main entry point: render the entire UI for the current app state.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = shell_layout(area);

    // Title bar
    let user = match &app.state {
        AppState::Running(r) => r.authenticated_user.as_ref(),
        _ => None,
    };
    let stats = match &app.state {
        AppState::Running(r) => r.tab_states.dashboard.stats.as_ref(),
        _ => None,
    };
    let title_lines = title_bar(user, stats);
    let title_widget = Paragraph::new(title_lines)
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title_widget, chunks[0]);

    // Tab bar (only when running)
    match &app.state {
        AppState::Running(r) => {
            let tab_line = tab_bar(r.active_tab);
            let tab_widget = Paragraph::new(tab_line)
                .block(Block::default().borders(Borders::BOTTOM));
            f.render_widget(tab_widget, chunks[1]);

            // Main content per active tab
            render_active_tab(f, chunks[2], app);

            // Key bar
            let bindings: Vec<(&str, &str)> = match r.active_tab {
                git_vacuum_app::state::TabKind::Dashboard => vec![
                    ("r", "refresh"), ("s", "sync"), ("?", "help"), ("q", "quit"),
                ],
                git_vacuum_app::state::TabKind::Explorer => vec![
                    ("↑↓", "navigate"), ("Space", "toggle"), ("Enter", "sync"),
                    ("Ctrl+A", "all"), ("/", "filter"), ("?", "help"),
                ],
                git_vacuum_app::state::TabKind::SyncCenter => vec![
                    ("p", "pause"), ("r", "resume"), ("c", "cancel"),
                    ("?", "help"), ("q", "quit"),
                ],
                git_vacuum_app::state::TabKind::ActivityLog => vec![
                    ("Enter", "view"), ("r", "refresh"), ("?", "help"),
                ],
                git_vacuum_app::state::TabKind::Settings => vec![
                    ("Tab", "category"), ("Enter", "edit"),
                    ("Ctrl+S", "save"), ("Esc", "discard"), ("?", "help"),
                ],
            };
            f.render_widget(
                Paragraph::new(key_bar(&bindings))
                    .block(Block::default().borders(Borders::TOP)),
                chunks[3],
            );
        }
        AppState::Auth(_) => {
            // No tab bar; render auth screen
            render_auth_screen(f, chunks[2], app);
            let bindings: Vec<(&str, &str)> = vec![("Enter", "submit"), ("Esc", "quit")];
            f.render_widget(
                Paragraph::new(key_bar(&bindings))
                    .block(Block::default().borders(Borders::TOP)),
                chunks[3],
            );
        }
        AppState::FatalError(msg) => {
            let p = Paragraph::new(format!("\n  FATAL: {msg}\n\n  Press q to quit."))
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: false });
            f.render_widget(p, chunks[2]);
        }
    }
}

fn render_active_tab(f: &mut Frame, area: Rect, app: &App) {
    let AppState::Running(state) = &app.state else { return };
    let area = centered(area);
    match state.active_tab {
        git_vacuum_app::state::TabKind::Dashboard => {
            render_dashboard(f, area, &state.tab_states.dashboard);
        }
        git_vacuum_app::state::TabKind::Explorer => {
            render_explorer(f, area, &state.tab_states.explorer, &state.repos);
        }
        git_vacuum_app::state::TabKind::SyncCenter => {
            render_sync_center(f, area, &state.tab_states.sync_center);
        }
        git_vacuum_app::state::TabKind::ActivityLog => {
            render_activity_log(f, area, &state.tab_states.activity_log);
        }
        git_vacuum_app::state::TabKind::Settings => {
            render_settings(f, area, &state.tab_states.settings);
        }
    }
}

fn render_auth_screen(f: &mut Frame, area: Rect, app: &App) {
    let AppState::Auth(auth) = &app.state else { return };
    let area = centered(area);
    render_auth(f, area, auth.mode, &auth.token_input, auth.error.as_deref(), auth.loading);
}

fn centered(area: Rect) -> Rect {
    if area.width < 80 {
        area
    } else {
        let pad = (area.width - 80) / 2;
        Rect {
            x: area.x + pad,
            y: area.y,
            width: 80,
            height: area.height,
        }
    }
}

/// Render a centered modal overlay (used by help, error detail, etc.)
#[allow(dead_code)]
pub fn render_modal(f: &mut Frame, area: Rect, title: &str, body: &str) {
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = centered(area);
    f.render_widget(Clear, inner);
    let p = Paragraph::new(body)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

/// Draw a "loading" placeholder.
#[allow(dead_code)]
pub fn render_loading(f: &mut Frame, area: Rect, what: &str) {
    let p = Paragraph::new(format!("  {what}..."))
        .block(Block::default().borders(Borders::ALL))
        .style(ratatui::style::Style::default().fg(COLOR_MUTED));
    f.render_widget(p, area);
}

/// Color constants re-exported for testing/screens.
pub use theme::{COLOR_ERROR, COLOR_SUCCESS, COLOR_WARNING};

// Helper to drop the unused import warning for COLOR_PRIMARY in lib.rs
#[allow(dead_code)]
const _UNUSED: ratatui::style::Color = COLOR_PRIMARY;
