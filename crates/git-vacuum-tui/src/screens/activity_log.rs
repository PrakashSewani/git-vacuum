use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::theme::COLOR_MUTED;
use git_vacuum_app::tabs::ActivityLogTabState;

pub fn render_activity_log(f: &mut Frame, area: Rect, state: &ActivityLogTabState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0), // run list
            Constraint::Length(2),
        ])
        .margin(1)
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Activity Log — Sync Run History ");
    let items: Vec<ListItem> = state
        .runs
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let style = if Some(i) == state.selected_run {
                Style::default().bg(ratatui::style::Color::DarkGray)
            } else {
                Style::default()
            };
            let line = Line::from(format!(
                " #{}  {}  {} cloned  {} updated  {} failed  [{}]",
                r.id,
                r.started_at.format("%Y-%m-%d %H:%M"),
                r.cloned_count,
                r.updated_count,
                r.failed_count,
                r.status
            ));
            ListItem::new(line).style(style)
        })
        .collect();
    let list = List::new(items).block(block);
    f.render_widget(list, chunks[0]);

    let status = Paragraph::new(Line::from(Span::styled(
        " Activity Log keeps a record of every sync run for review and troubleshooting.   Enter: view  r: refresh",
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(status, chunks[1]);
}
