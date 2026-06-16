pub mod input;
pub mod terminal;

use git_vacuum_app::{App, AppState, Modal, SyncPhase};
use git_vacuum_app::LogEntryStatus;
use git_vacuum_core::Tab;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

pub fn render(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    if area.width < 80 || area.height < 24 {
        render_too_small(frame, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let title_area = chunks[0];
    let tab_area = chunks[1];
    let content_area = chunks[2];
    let key_bar_area = chunks[3];

    render_title_bar(frame, title_area, app);
    render_tab_bar(frame, tab_area, app);
    render_content(frame, content_area, app);
    render_key_bar(frame, key_bar_area, app);

    if let AppState::Running(state) = &app.state {
        if let Some(modal) = state.modal_stack.last() {
            render_modal(frame, area, modal);
        }
    }
}

fn render_too_small(frame: &mut ratatui::Frame, area: Rect) {
    let msg = Paragraph::new("Terminal too small. Please resize to at least 80x24.")
        .block(Block::default().borders(Borders::ALL).title("git-vacuum"))
        .centered();
    frame.render_widget(msg, area);
}

fn render_title_bar(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let text = match &app.state {
        AppState::Auth(_) => "git-vacuum v0.1.0 | Authenticate to continue".to_string(),
        AppState::Running(state) => {
            let status = match state.tabs.sync_center.phase {
                SyncPhase::Active => "⣾ Syncing...",
                SyncPhase::Paused => "⏸ Paused",
                _ => "Ready",
            };
            format!(
                "git-vacuum | {} | {} repos | {}",
                state.username,
                state.repos.len(),
                status,
            )
        }
        AppState::FatalError { .. } => "git-vacuum | ERROR".to_string(),
    };

    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(p, area);
}

fn render_tab_bar(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let tabs = vec!["Dashboard", "Explorer", "Sync Center", "Activity Log", "Settings"];
    let active_idx = match &app.state {
        AppState::Running(state) => state.active_tab as usize,
        _ => 0,
    };

    let titles: Vec<Line> = tabs.iter().enumerate().map(|(i, t)| {
        if i == active_idx {
            Line::from(Span::styled(
                format!(" ▸ {} ", t),
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Black)
                    .bg(ratatui::style::Color::White),
            ))
        } else {
            Line::from(Span::styled(
                format!("   {}   ", t),
                ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
            ))
        }
    }).collect();

    let tabs_widget = Tabs::new(titles).block(Block::default());
    frame.render_widget(tabs_widget, area);
}

fn render_content(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    match &app.state {
        AppState::Auth(state) => render_auth_screen(frame, area, state),
        AppState::Running(state) => {
            match state.active_tab {
                Tab::Dashboard => render_dashboard(frame, area, state),
                Tab::Explorer => render_explorer(frame, area, state),
                Tab::SyncCenter => render_sync_center(frame, area, state),
                Tab::ActivityLog => render_activity_log(frame, area, state),
                Tab::Settings => render_settings(frame, area, state),
            }
        }
        AppState::FatalError { message } => {
            let p = Paragraph::new(message.as_str())
                .block(Block::default().borders(Borders::ALL).title("Fatal Error"))
                .centered();
            frame.render_widget(p, area);
        }
    }
}

fn render_key_bar(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let keys = match &app.state {
        AppState::Auth(_) => "s:Submit | q:Quit | ?:Help",
        AppState::Running(state) => {
            match state.active_tab {
                Tab::Dashboard => "r:Refresh  s:Sync  Tab:Next  q:Quit  ?:Help",
                Tab::Explorer => "Space:Toggle  r:Refresh  s:Sync  /:Filter  Tab:Next  q:Quit  ?:Help",
                Tab::SyncCenter => match state.tabs.sync_center.phase {
                    SyncPhase::PreSync { .. } => "Enter:Start  Tab:Next  q:Quit",
                    SyncPhase::Active => "p:Pause  c:Cancel  f:Follow  Tab:Next  q:Quit",
                    SyncPhase::Paused => "r:Resume  c:Cancel  Tab:Next  q:Quit",
                    _ => "Tab:Next  q:Quit",
                },
                Tab::ActivityLog => "Enter:Detail  Tab:Next  q:Quit  ?:Help",
                Tab::Settings => "Tab:Next  q:Quit  ?:Help",
            }
        }
        _ => "q:Quit",
    };

    let p = Paragraph::new(keys)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(p, area);
}

fn render_modal(frame: &mut ratatui::Frame, area: Rect, modal: &Modal) {
    let modal_area = centered_rect(60, 20, area);
    let (title, content) = match modal {
        Modal::Confirmation { title, message, confirm_label, cancel_label, danger, .. } => {
            let color = if *danger {
                ratatui::style::Color::Red
            } else {
                ratatui::style::Color::Yellow
            };
            (title.as_str(), vec![
                Line::from(""),
                Line::from(message.as_str()),
                Line::from(""),
                Line::from(format!(" [Enter] {}  [Esc] {}", confirm_label, cancel_label)),
            ])
        }
        Modal::Help { .. } => ("Keyboard Shortcuts", vec![
            Line::from("1-5: Switch tabs | Tab: Next tab | q: Quit | ?: Help"),
            Line::from("Space: Toggle repo | r: Refresh | s: Sync"),
            Line::from("p: Pause sync | c: Cancel sync | Enter: Confirm"),
        ]),
        Modal::ErrorDetail { repo_full_name, error_message, .. } => {
            ("Error Detail", vec![
                Line::from(format!("Repository: {}", repo_full_name)),
                Line::from(""),
                Line::from(error_message.as_str()),
            ])
        }
        Modal::RepoDetail { repo_index, .. } => {
            ("Repository Detail", vec![
                Line::from(format!("Index: {}", repo_index)),
                Line::from("(Details coming in full implementation)"),
            ])
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan));

    let p = Paragraph::new(content).block(block);
    frame.render_widget(p, modal_area);
}

fn render_auth_screen(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &git_vacuum_app::AuthScreenState,
) {
    let mut content = vec![
        Line::from(""),
        Line::from("  Connect your GitHub account to discover and sync your repositories."),
        Line::from(""),
        Line::from("  Enter your GitHub Personal Access Token:"),
        Line::from(""),
        Line::from(format!("  {}", "*".repeat(state.token_input.len()))),
        Line::from(""),
        Line::from("  Token needs: repo, read:org scope"),
        Line::from(""),
    ];

    if let Some(status) = &state.status_message {
        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::styled("  ", ratatui::style::Style::default()),
            Span::styled(status.as_str(), ratatui::style::Style::default().fg(ratatui::style::Color::Yellow)),
        ]));
    }

    if let Some((_reason, detail)) = &state.error {
        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::styled("  ERROR: ", ratatui::style::Style::default().fg(ratatui::style::Color::Red)),
            Span::styled(detail.as_str(), ratatui::style::Style::default()),
        ]));
    }

    let extra_lines = if state.error.is_some() { 2 } else if state.status_message.is_some() { 2 } else { 0 };
    let msg_area = centered_rect(60, content.len() as u16 + 3 + extra_lines, area);
    let title = if state.error.is_some() { "Authentication" } else { "Git-Vacuum Authentication" };
    let block = Block::default().borders(Borders::ALL).title(title);
    let p = Paragraph::new(content).block(block);
    frame.render_widget(p, msg_area);
}

fn render_explorer(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &git_vacuum_app::RunningAppState,
) {
    let explorer = &state.tabs.explorer;
    let mut lines: Vec<Line> = vec![
        Line::from(format!("Source: {:?} | Filter: \"{}\" | Skip archived: {} | Skip forks: {}",
            explorer.source, explorer.filter_text, explorer.skip_archived, explorer.skip_forks)),
        Line::from(""),
        Line::from(format!("  {:<4} {:<30} {:<15} {:>8}", "#", "Name", "Owner", "Status")),
        Line::from(format!("  {:-<4} {:-<30} {:-<15} {:-<8}", "", "", "", "")),
    ];

    for (i, repo) in state.repos.iter().enumerate() {
        if i >= 20 { break; }
        let selected = if state.selected_indices.contains(&i) { "✓" } else { " " };
        let status = match repo.clone_status {
            git_vacuum_core::CloneStatus::Cloned => "cloned",
            git_vacuum_core::CloneStatus::Stale => "stale",
            git_vacuum_core::CloneStatus::Error => "error",
            _ => "-",
        };
        lines.push(Line::from(format!(
            "  [{:<1}] {:<30} {:<15} {:>8}",
            selected,
            truncate(&repo.name, 30),
            truncate(&repo.owner_login, 15),
            status,
        )));
    }

    let block = Block::default().borders(Borders::ALL).title("Repository Explorer");
    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_dashboard(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &git_vacuum_app::RunningAppState,
) {
    let dash = &state.tabs.dashboard;
    let lines = vec![
        Line::from(""),
        Line::from(format!("  Total repos:     {}", state.repos.len())),
        Line::from(format!("  On disk:         {} ", git_vacuum_core::human_bytes(dash.total_size_bytes))),
        Line::from(format!("  Up to date:      {}", dash.up_to_date)),
        Line::from(format!("  Behind remote:   {}", dash.behind)),
        Line::from(format!("  With errors:     {}", dash.errors)),
        Line::from(""),
        Line::from("  ── Repos Needing Attention ──"),
    ];

    let mut attention_lines = Vec::new();
    for repo in &dash.attention_list {
        attention_lines.push(Line::from(format!("    {}", repo.full_name)));
    }
    let all_lines: Vec<Line> = lines.into_iter().chain(attention_lines).collect();

    let block = Block::default().borders(Borders::ALL).title("Dashboard");
    let p = Paragraph::new(all_lines).block(block);
    frame.render_widget(p, area);
}

fn render_sync_center(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &git_vacuum_app::RunningAppState,
) {
    let sync = &state.tabs.sync_center;
    let lines = match &sync.phase {
        SyncPhase::Idle => vec![
            Line::from(""),
            Line::from("  No sync in progress."),
            Line::from("  Go to Explorer, select repos, and press 's' to sync."),
        ],
        SyncPhase::PreSync { clone_count, sync_count } => vec![
            Line::from(""),
            Line::from(format!("  Repos to clone:  {}", clone_count)),
            Line::from(format!("  Repos to sync:   {}", sync_count)),
            Line::from(""),
            Line::from("  Press Enter to start sync."),
        ],
        SyncPhase::Active => {
            let mut lines = vec![
                Line::from(""),
                Line::from(format!("  Progress: {}/{} repos | {:.1}%",
                    sync.progress_done, sync.progress_total,
                    if sync.progress_total > 0 {
                        (sync.progress_done as f32 / sync.progress_total as f32) * 100.0
                    } else { 0.0 }
                )),
                Line::from(format!("  Data: {} transferred", git_vacuum_core::human_bytes(sync.bytes_done))),
                Line::from(""),
            ];
            for entry in &sync.live_log {
                let icon = match entry.status {
                    LogEntryStatus::Success => "✓",
                    LogEntryStatus::Failed => "✗",
                    LogEntryStatus::Active => "⣾",
                    LogEntryStatus::Queued => "—",
                    LogEntryStatus::Skipped => "○",
                };
                lines.push(Line::from(format!("  {} {} — {}", icon, entry.repo_full_name, entry.detail)));
            }
            lines
        }
        SyncPhase::Paused => vec![
            Line::from(""),
            Line::from("  ⏸ Sync PAUSED"),
            Line::from("  Press r to resume or c to cancel."),
        ],
        SyncPhase::Completed => vec![
            Line::from(""),
            Line::from("  ✓ Sync Complete!"),
            Line::from(""),
            Line::from(format!("  {}/{} repos processed successfully.",
                sync.progress_done, sync.progress_total)),
        ],
        SyncPhase::Cancelled => vec![
            Line::from(""),
            Line::from("  Sync cancelled."),
        ],
    };

    let block = Block::default().borders(Borders::ALL).title("Sync Center");
    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_activity_log(
    frame: &mut ratatui::Frame,
    area: Rect,
    _state: &git_vacuum_app::RunningAppState,
) {
    let lines = vec![
        Line::from(""),
        Line::from("  No sync history yet."),
        Line::from("  Run your first sync to see activity here."),
    ];
    let block = Block::default().borders(Borders::ALL).title("Activity Log");
    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_settings(
    frame: &mut ratatui::Frame,
    area: Rect,
    _state: &git_vacuum_app::RunningAppState,
) {
    let lines = vec![
        Line::from(""),
        Line::from("  Settings will be available in v1.0"),
        Line::from("  Configuration via CLI flags and environment variables for MVP."),
    ];
    let block = Block::default().borders(Borders::ALL).title("Settings");
    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area.width * percent_x / 100;
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    Rect::new(x, y, width, height)
}

fn truncate(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else {
        format!("{}...", &s[..max_width - 3])
    }
}
