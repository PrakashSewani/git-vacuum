use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use ratatui::Frame;

use git_vacuum_app::tabs::DashboardTabState;
use crate::components::{progress_bar, spinner_frame};
use crate::theme::{
    COLOR_ERROR, COLOR_ERROR_BRIGHT, COLOR_MUTED, COLOR_PRIMARY, COLOR_PRIMARY_BRIGHT,
    COLOR_SUCCESS, COLOR_SUCCESS_BRIGHT, COLOR_WARNING, COLOR_WARNING_BRIGHT,
};

pub fn render_dashboard(f: &mut Frame, area: Rect, state: &DashboardTabState, tick: u64) {
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
        let up_pct = (s.up_to_date as f64 / total as f64 * 100.0) as u16;
        let label = format!("{}/{} up to date", s.up_to_date, s.total_repos);
        let gauge = Gauge::default()
            .block(health_block)
            .gauge_style(Style::default().fg(COLOR_SUCCESS_BRIGHT).add_modifier(Modifier::BOLD))
            .percent(up_pct.min(100))
            .label(label);
        f.render_widget(gauge, chunks[0]);
    } else {
        // Skeleton: animated progress bar
        let pulse = (tick as f32 / 20.0).sin();
        let pct = 0.3 + (pulse * 0.2);
        let width = (chunks[0].width as usize).saturating_sub(6);
        let bar = progress_bar(width, pct, tick);
        let placeholder = Paragraph::new(vec![
            Line::from(Span::styled(
                " Dashboard: overview of local backup mirror health.",
                Style::default().fg(COLOR_MUTED),
            )),
            Line::from(Span::styled(
                format!(" {} Computing stats", spinner_frame(tick)),
                Style::default().fg(COLOR_MUTED),
            )),
            Line::from(Span::styled(bar, Style::default().fg(COLOR_PRIMARY_BRIGHT))),
        ])
        .block(health_block);
        f.render_widget(placeholder, chunks[0]);
    }

    // Quick stats
    let stats_block = Block::default().borders(Borders::ALL).title(" Quick Stats ");
    if let Some(s) = state.stats.as_ref() {
        let line = Line::from(vec![
            Span::raw("◉ Total repos: "),
            Span::styled(format!("{}", s.total_repos), Style::default().fg(COLOR_PRIMARY_BRIGHT).add_modifier(Modifier::BOLD)),
            Span::raw("   ✓ Synced: "),
            Span::styled(format!("{}", s.up_to_date), Style::default().fg(COLOR_SUCCESS_BRIGHT)),
            Span::raw("   ⚠ Behind: "),
            Span::styled(format!("{}", s.behind), Style::default().fg(COLOR_WARNING_BRIGHT)),
            Span::raw("   ✗ Errors: "),
            Span::styled(format!("{}", s.errors), Style::default().fg(COLOR_ERROR_BRIGHT)),
        ]);
        let stats = Paragraph::new(line).block(stats_block);
        f.render_widget(stats, chunks[1]);
    } else {
        let line = Line::from(Span::styled(
            " Dashboard shows the current state after your most recent sync run.",
            Style::default().fg(COLOR_MUTED),
        ));
        f.render_widget(Paragraph::new(line).block(stats_block), chunks[1]);
    }

    // Attention list
    let att_block = Block::default().borders(Borders::ALL).title(" Repos Needing Attention ");
    if state.attention_list.is_empty() && state.stats.is_none() {
        // Skeleton rows
        let items: Vec<ListItem> = (0..4).map(|i| {
            let offset = (i + (tick / 4) as usize) % 4;
            let shimmer = match offset {
                0 => "▓▓▓▓▓▓▓▓▓▓",
                1 => "▒▓▓▓▓▓▓▓▓▓",
                2 => "░▒▓▓▓▓▓▓▓▓",
                _ => "░░▒▓▓▓▓▓▓▓",
            };
            let label = match i {
                0 => "████████████████████",
                1 => "██████████████",
                2 => "████████████████",
                _ => "██████████████████",
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<30}", label), Style::default().fg(COLOR_MUTED)),
                Span::styled(shimmer, Style::default().fg(COLOR_PRIMARY_BRIGHT)),
            ]))
        }).collect();
        let list = List::new(items).block(att_block);
        f.render_widget(list, chunks[2]);
    } else {
        let items: Vec<ListItem> = state
            .attention_list
            .iter()
            .map(|item| {
                let (color, icon) = if item.reason == "error" { (COLOR_ERROR_BRIGHT, "✗") }
                           else if item.reason == "stale" { (COLOR_WARNING_BRIGHT, "⚠") }
                           else { (COLOR_SUCCESS_BRIGHT, "✓") };
                ListItem::new(Line::from(Span::styled(
                    format!(" {} {} — {}", icon, item.full_name, item.detail),
                    Style::default().fg(color),
                )))
            })
            .collect();
        let list = List::new(items).block(att_block);
        f.render_widget(list, chunks[2]);
    }

    // Suppress unused warnings
    let _ = COLOR_PRIMARY;
    let _ = COLOR_SUCCESS;
    let _ = COLOR_WARNING;
    let _ = COLOR_ERROR;
}
