use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::tabs::DashboardTabState;
use crate::theme::{COLOR_ERROR, COLOR_MUTED, COLOR_PRIMARY, COLOR_SUCCESS, COLOR_WARNING};

pub fn render_dashboard(f: &mut Frame, area: Rect, state: &DashboardTabState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // sync health
            Constraint::Length(3),  // quick stats
            Constraint::Min(0),     // attention list
        ])
        .margin(1)
        .split(area);

    // Sync health panel (gauge + counts)
    let health_block = Block::default().borders(Borders::ALL).title(" Sync Health ");
    if let Some(s) = state.stats.as_ref() {
        let total = s.total_repos.max(1) as u64;
        let up_pct = (s.up_to_date as f64 / total as f64 * 100.0) as u8;
        let label = format!("{}/{} up to date", s.up_to_date, s.total_repos);
        let gauge = Gauge::default()
            .block(health_block)
            .gauge_style(Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD))
            .percent(up_pct.min(100) as u16)
            .label(label);
        f.render_widget(gauge, chunks[0]);
    } else {
        let placeholder = Paragraph::new(Line::from(Span::styled(
            " Loading...",
            Style::default().fg(COLOR_MUTED),
        )))
        .block(health_block);
        f.render_widget(placeholder, chunks[0]);
    }

    // Quick stats
    let stats_block = Block::default().borders(Borders::ALL).title(" Quick Stats ");
    if let Some(s) = state.stats.as_ref() {
        let line = Line::from(vec![
            Span::raw(format!("Total repos: {}   ", s.total_repos)),
            Span::styled(format!("↑{} behind ", s.behind), Style::default().fg(COLOR_WARNING)),
            Span::styled(format!("✗{} errors", s.errors), Style::default().fg(COLOR_ERROR)),
        ]);
        let stats = Paragraph::new(line).block(stats_block);
        f.render_widget(stats, chunks[1]);
    } else {
        f.render_widget(Paragraph::new("").block(stats_block), chunks[1]);
    }

    // Attention list
    let att_block = Block::default().borders(Borders::ALL).title(" Repos Needing Attention ");
    let items: Vec<ListItem> = state
        .attention_list
        .iter()
        .map(|item| {
            let color = if item.reason == "error" { COLOR_ERROR }
                       else if item.reason == "stale" { COLOR_WARNING }
                       else { COLOR_SUCCESS };
            ListItem::new(Line::from(Span::styled(
                format!(" {} — {}", item.full_name, item.detail),
                Style::default().fg(color),
            )))
        })
        .collect();
    let list = List::new(items).block(att_block);
    f.render_widget(list, chunks[2]);
}
