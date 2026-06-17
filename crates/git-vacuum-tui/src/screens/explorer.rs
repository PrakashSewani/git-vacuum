use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::tabs::ExplorerTabState;
use git_vacuum_core::RepoEntry;
use crate::components::{format_repo_row, highlight_style};
use crate::theme::{COLOR_MUTED, COLOR_PRIMARY};

pub fn render_explorer(
    f: &mut Frame,
    area: Rect,
    state: &ExplorerTabState,
    repos: &[RepoEntry],
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

    // Source selector header
    let source_str = match &state.source {
        git_vacuum_core::RepoSource::MyRepos => "My Repos".to_string(),
        git_vacuum_core::RepoSource::Org { login } => format!("Org: {login}"),
        git_vacuum_core::RepoSource::Starred => "Starred".to_string(),
        git_vacuum_core::RepoSource::All => "All Accessible".to_string(),
    };
    let header_text = format!(
        " Source: {}   Filter: {}{}   r: refresh  Enter: sync  /: filter  Space: select",
        source_str,
        state.filter_text,
        if state.loading { "  (loading...)" } else { "" }
    );
    let header = Paragraph::new(Line::from(Span::styled(header_text, Style::default().fg(COLOR_PRIMARY))))
        .block(Block::default().borders(Borders::ALL).title(" Source "));
    f.render_widget(header, chunks[0]);

    // Repo table
    let title = if state.loading && repos.is_empty() {
        " Repos (loading...) "
    } else {
        &format!(" Repos ({}) ", repos.len())
    };
    let items: Vec<ListItem> = if repos.is_empty() {
        let msg = if state.loading {
            vec![Line::from(Span::styled("  Loading repositories from GitHub...", Style::default().fg(COLOR_MUTED)))]
        } else {
            vec![Line::from(Span::styled("  No repositories found. Press 'r' to refresh.", Style::default().fg(COLOR_MUTED)))]
        };
        vec![ListItem::new(Line::from(""))]
            .into_iter()
            .chain(msg.into_iter().map(ListItem::new))
            .collect()
    } else {
        repos
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let line = format_repo_row(repos, idx, chunks[1].width as usize);
                let style = if idx == state.cursor { highlight_style() } else { Style::default() };
                ListItem::new(line).style(style)
            })
            .collect()
    };
    let table = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(table, chunks[1]);

    // Status
    let status = Paragraph::new(Line::from(Span::styled(
        " ↑/↓: navigate  Space: toggle  Enter: start sync  Ctrl+A: select all  Ctrl+D: deselect all",
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(status, chunks[2]);
}
