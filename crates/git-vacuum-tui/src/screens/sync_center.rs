use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::tabs::{LogStatus, SyncCenterTabState, SyncPhase};
use crate::theme::{COLOR_ERROR, COLOR_MUTED, COLOR_PRIMARY, COLOR_SUCCESS, COLOR_WARNING};

pub fn render_sync_center(f: &mut Frame, area: Rect, state: &SyncCenterTabState) {
    match &state.phase {
        SyncPhase::Idle => render_idle(f, area),
        SyncPhase::PreSync => render_pre(f, area),
        SyncPhase::Active | SyncPhase::Paused => render_active(f, area, state),
        SyncPhase::Completed(summary) => render_completed(f, area, summary),
        SyncPhase::Cancelled(summary) => render_cancelled(f, area, summary),
    }
}

fn render_idle(f: &mut Frame, area: Rect) {
    let p = Paragraph::new(Line::from(Span::styled(
        " No sync running. Press Enter in the Explorer to start one.",
        Style::default().fg(COLOR_MUTED),
    )))
    .block(Block::default().borders(Borders::ALL).title(" Sync Center "));
    f.render_widget(p, area);
}

fn render_pre(f: &mut Frame, area: Rect) {
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(" Ready to sync. Press Enter to confirm, Esc to cancel."),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Pre-Sync "));
    f.render_widget(p, area);
}

fn render_active(f: &mut Frame, area: Rect, state: &SyncCenterTabState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // overall progress
            Constraint::Min(0),     // log
        ])
        .margin(1)
        .split(area);

    // Overall progress
    let progress_block = Block::default().borders(Borders::ALL).title(" Overall Progress ");
    if let Some(p) = state.overall.as_ref() {
        let pct = p.percent.clamp(0.0, 100.0) as u8;
        let label = format!("{} / {} repos", p.completed, p.total_jobs);
        let gauge = Gauge::default()
            .block(progress_block)
            .gauge_style(Style::default().fg(COLOR_PRIMARY))
            .percent(pct as u16)
            .label(label);
        f.render_widget(gauge, chunks[0]);
    } else {
        f.render_widget(Paragraph::new("").block(progress_block), chunks[0]);
    }

    // Log
    let log_block = Block::default().borders(Borders::ALL).title(" Live Log ");
    let items: Vec<ListItem> = state
        .live_log
        .iter()
        .map(|e| {
            let color = match e.status {
                LogStatus::Queued => COLOR_MUTED,
                LogStatus::Active => COLOR_PRIMARY,
                LogStatus::Success => COLOR_SUCCESS,
                LogStatus::Failed => COLOR_ERROR,
                LogStatus::Skipped => COLOR_WARNING,
            };
            ListItem::new(Line::from(Span::styled(
                format!(" {}  {}", e.repo_full_name, e.detail),
                Style::default().fg(color),
            )))
        })
        .collect();
    let list = List::new(items).block(log_block);
    f.render_widget(list, chunks[1]);
}

fn render_completed(f: &mut Frame, area: Rect, s: &git_vacuum_core::SyncSummary) {
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(format!(" Total: {} repos", s.total_jobs), Style::default().fg(COLOR_PRIMARY))),
        Line::from(Span::styled(format!(" Cloned: {}", s.cloned), Style::default().fg(COLOR_SUCCESS))),
        Line::from(Span::styled(format!(" Updated: {}", s.updated), Style::default().fg(COLOR_SUCCESS))),
        Line::from(Span::styled(format!(" Up-to-date: {}", s.up_to_date), Style::default().fg(COLOR_MUTED))),
        Line::from(Span::styled(format!(" Failed: {}", s.failed), Style::default().fg(COLOR_ERROR))),
        Line::from(Span::styled(format!(" Bytes transferred: {}", s.bytes_transferred), Style::default().fg(COLOR_MUTED))),
        Line::from(Span::styled(format!(" Duration: {:?}", s.duration), Style::default().fg(COLOR_MUTED))),
    ];
    let p = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(" Sync Completed "))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_cancelled(f: &mut Frame, area: Rect, s: &git_vacuum_core::PartialSyncSummary) {
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(" Sync was cancelled.", Style::default().fg(COLOR_WARNING))),
        Line::from(Span::styled(format!(" Completed: {}", s.completed), Style::default().fg(COLOR_SUCCESS))),
        Line::from(Span::styled(format!(" Failed: {}", s.failed), Style::default().fg(COLOR_ERROR))),
        Line::from(Span::styled(format!(" Cancelled mid-operation: {}", s.cancelled), Style::default().fg(COLOR_WARNING))),
        Line::from(Span::styled(format!(" Pending dropped: {}", s.pending_dropped), Style::default().fg(COLOR_MUTED))),
    ];
    let p = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(" Sync Cancelled "))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
