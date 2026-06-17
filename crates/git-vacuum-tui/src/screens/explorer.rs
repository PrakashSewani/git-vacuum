use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::tabs::ExplorerTabState;
use git_vacuum_core::RepoEntry;
use crate::components::{format_repo_row, highlight_style, spinner_frame};
use crate::theme::{COLOR_MUTED, COLOR_PRIMARY, COLOR_PRIMARY_BRIGHT};

pub fn render_explorer(
    f: &mut Frame,
    area: Rect,
    state: &ExplorerTabState,
    repos: &[RepoEntry],
    tick: u64,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // source selector + filters
            Constraint::Min(0),     // repo table
            Constraint::Length(2),  // status line
        ])
        .margin(1)
        .split(area);

    // Source selector header — chip-style
    let source_str = match &state.source {
        git_vacuum_core::RepoSource::MyRepos => "My Repos".to_string(),
        git_vacuum_core::RepoSource::Org { login } => format!("Org: {login}"),
        git_vacuum_core::RepoSource::Starred => "Starred".to_string(),
        git_vacuum_core::RepoSource::All => "All Accessible".to_string(),
    };
    let source_chip = Span::styled(
        format!(" {} ", source_str),
        Style::default().fg(COLOR_PRIMARY_BRIGHT).bg(ratatui::style::Color::Rgb(30, 30, 44)),
    );
    let filter_chip = if state.filter_text.is_empty() {
        Span::styled("  Filter: (none)  ", Style::default().fg(COLOR_MUTED))
    } else {
        Span::styled(
            format!("  Filter: {}  ", state.filter_text),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        )
    };
    let loading_chip = if state.loading {
        Span::styled(
            format!("  {} ", spinner_frame(tick)),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        )
    } else {
        Span::raw("")
    };
    let header = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        source_chip,
        filter_chip,
        loading_chip,
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Source "));
    f.render_widget(header, chunks[0]);

    // Repo table
    let title = if state.loading && repos.is_empty() {
        " Repos (loading...) "
    } else {
        &format!(" Repos ({}) ", repos.len())
    };
    let items: Vec<ListItem> = if repos.is_empty() {
        if state.loading {
            // Animated skeleton rows
            (0..6).map(|i| {
                let shimmer_phase = (i as u64 + tick / 3) % 6;
                let shimmer = match shimmer_phase {
                    0 => "▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓",
                    1 => "▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓",
                    2 => "░▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓",
                    3 => "░░▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓",
                    4 => "░░░▒▓▓▓▓▓▓▓▓▓▓▓▓▓▓",
                    _ => "░░░░▒▓▓▓▓▓▓▓▓▓▓▓▓▓",
                };
                let name_pad = match i {
                    0 => "████████████████████",
                    1 => "██████████████",
                    2 => "████████████████████████",
                    3 => "████████████",
                    4 => "██████████████████████",
                    _ => "██████████████████",
                };
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(name_pad, Style::default().fg(COLOR_MUTED)),
                    Span::raw("   "),
                    Span::styled(shimmer, Style::default().fg(COLOR_PRIMARY_BRIGHT)),
                ]))
            }).collect()
        } else {
            vec![ListItem::new(Line::from(Span::styled(
                "  No repositories found. Press 'r' to refresh.",
                Style::default().fg(COLOR_MUTED),
            )))]
        }
    } else {
        repos
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let line = format_repo_row(repos, idx, chunks[1].width as usize);
                let is_selected = idx == state.cursor;
                // Pulse the selected row's left edge: alternate '>' and '█'
                let mut style = if is_selected {
                    highlight_style()
                } else {
                    Style::default()
                };
                if is_selected {
                    style = style.add_modifier(Modifier::BOLD);
                }
                let prefix = if is_selected {
                    let pulse = if (tick / 3) % 2 == 0 { "▶" } else { "▸" };
                    format!(" {} ", pulse)
                } else {
                    "   ".to_string()
                };
                let row_text: String = format!("{}{}", prefix, line);
                ListItem::new(Line::from(Span::styled(
                    row_text,
                    if is_selected { style } else { Style::default() },
                ))).style(if is_selected { style } else { Style::default() })
            })
            .collect()
    };
    let table = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(table, chunks[1]);

    // Status
    let status = Paragraph::new(Line::from(Span::styled(
        " ↑/↓: navigate  Space: toggle  Enter: start sync  Ctrl+A: all  Ctrl+D: none  /: filter  ?: help",
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(status, chunks[2]);

    // Suppress unused
    let _ = COLOR_PRIMARY;
}
