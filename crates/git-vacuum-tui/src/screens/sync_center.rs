use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use ratatui::Frame;

use git_vacuum_app::tabs::{LogStatus, SyncCenterTabState, SyncPhase};
use crate::components::{progress_bar, spinner_frame};
use crate::theme::{
    COLOR_ACCENT, COLOR_ERROR, COLOR_ERROR_BRIGHT, COLOR_MUTED, COLOR_PRIMARY,
    COLOR_PRIMARY_BRIGHT, COLOR_SUCCESS, COLOR_SUCCESS_BRIGHT, COLOR_WARNING,
    COLOR_WARNING_BRIGHT,
};

pub fn render_sync_center(f: &mut Frame, area: Rect, state: &SyncCenterTabState, tick: u64) {
    match &state.phase {
        SyncPhase::Idle => render_idle(f, area, tick),
        SyncPhase::PreSync => render_pre(f, area, state, tick),
        SyncPhase::Active | SyncPhase::Paused => render_active(f, area, state, tick),
        SyncPhase::Completed(summary) => render_completed(f, area, summary),
        SyncPhase::Cancelled(summary) => render_cancelled(f, area, summary),
    }
}

fn render_idle(f: &mut Frame, area: Rect, tick: u64) {
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            " Sync Center: live view of clone / fetch operations.",
            Style::default().fg(COLOR_PRIMARY_BRIGHT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" {} No sync running.", spinner_frame(tick)),
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Select repos in Explorer and press Enter to start a sync.",
            Style::default().fg(COLOR_MUTED),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Sync Center "));
    f.render_widget(p, area);
}

fn render_pre(f: &mut Frame, area: Rect, state: &SyncCenterTabState, tick: u64) {
    let width = (area.width as usize).saturating_sub(8);
    let bar = progress_bar(width.max(16), 0.25 + (tick as f32 / 20.0).sin() * 0.15, tick);
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" {} Sync request in progress", spinner_frame(tick)),
            Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("   {} repos queued for backup", state.queued_repos),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        )),
        Line::from(Span::styled(
            format!("   Base path: {}", state.base_path.display()),
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(bar, Style::default().fg(COLOR_PRIMARY_BRIGHT))),
        Line::from(""),
        Line::from(Span::styled(
            " First repository will appear here shortly. Press Esc to cancel.",
            Style::default().fg(COLOR_MUTED),
        )),
    ];
    let p = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(" Pre-Sync "));
    f.render_widget(p, area);
}

fn render_active(f: &mut Frame, area: Rect, state: &SyncCenterTabState, tick: u64) {
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
        let pct = p.percent.clamp(0.0, 100.0) as u16;
        let label = format!("{} / {} repos", p.completed, p.total_jobs);
        let gauge = Gauge::default()
            .block(progress_block)
            .gauge_style(Style::default().fg(COLOR_PRIMARY_BRIGHT))
            .percent(pct.min(100))
            .label(label);
        f.render_widget(gauge, chunks[0]);
    } else {
        let width = (chunks[0].width as usize).saturating_sub(4);
        let bar = progress_bar(width, 0.0, tick);
        let placeholder = Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" {} Starting sync", spinner_frame(tick)),
                Style::default().fg(COLOR_ACCENT),
            )),
            Line::from(Span::styled(bar, Style::default().fg(COLOR_PRIMARY_BRIGHT))),
        ])
        .block(progress_block);
        f.render_widget(placeholder, chunks[0]);
    }

    // Log
    let log_block = Block::default().borders(Borders::ALL).title(" Live Log ");
    let items: Vec<ListItem> = state
        .live_log
        .iter()
        .map(|e| {
            let (color, icon) = match e.status {
                LogStatus::Queued => (COLOR_MUTED, "○"),
                LogStatus::Active => (COLOR_PRIMARY_BRIGHT, "◉"),
                LogStatus::Success => (COLOR_SUCCESS_BRIGHT, "✓"),
                LogStatus::Failed => (COLOR_ERROR_BRIGHT, "✗"),
                LogStatus::Skipped => (COLOR_WARNING_BRIGHT, "⊘"),
            };
            let prefix = if matches!(e.status, LogStatus::Active) {
                format!(" {} ", spinner_frame(tick))
            } else {
                format!(" {} ", icon)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}  {}", prefix, e.repo_full_name, e.detail),
                Style::default().fg(color),
            )))
        })
        .collect();
    let list = List::new(items).block(log_block);
    f.render_widget(list, chunks[1]);

    // Suppress unused
    let _ = COLOR_PRIMARY;
    let _ = COLOR_SUCCESS;
    let _ = COLOR_WARNING;
    let _ = COLOR_ERROR;
}

fn render_completed(f: &mut Frame, area: Rect, s: &git_vacuum_core::SyncSummary) {
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(format!(" ◉ Total: {} repos", s.total_jobs), Style::default().fg(COLOR_PRIMARY_BRIGHT).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("   ✓ Cloned: {}", s.cloned), Style::default().fg(COLOR_SUCCESS_BRIGHT))),
        Line::from(Span::styled(format!("   ✓ Updated: {}", s.updated), Style::default().fg(COLOR_SUCCESS_BRIGHT))),
        Line::from(Span::styled(format!("   · Up-to-date: {}", s.up_to_date), Style::default().fg(COLOR_MUTED))),
        Line::from(Span::styled(format!("   ✗ Failed: {}", s.failed), Style::default().fg(COLOR_ERROR_BRIGHT))),
        Line::from(Span::styled(format!("   Bytes: {}", s.bytes_transferred), Style::default().fg(COLOR_MUTED))),
        Line::from(Span::styled(format!("   Duration: {:?}", s.duration), Style::default().fg(COLOR_MUTED))),
    ];
    let p = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(" ✓ Sync Completed "));
    f.render_widget(p, area);
}

fn render_cancelled(f: &mut Frame, area: Rect, s: &git_vacuum_core::PartialSyncSummary) {
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(" Sync was cancelled.", Style::default().fg(COLOR_WARNING_BRIGHT).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("   Completed: {}", s.completed), Style::default().fg(COLOR_SUCCESS_BRIGHT))),
        Line::from(Span::styled(format!("   Failed: {}", s.failed), Style::default().fg(COLOR_ERROR_BRIGHT))),
        Line::from(Span::styled(format!("   Cancelled mid-operation: {}", s.cancelled), Style::default().fg(COLOR_WARNING_BRIGHT))),
        Line::from(Span::styled(format!("   Pending dropped: {}", s.pending_dropped), Style::default().fg(COLOR_MUTED))),
    ];
    let p = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(" ⊘ Sync Cancelled "));
    f.render_widget(p, area);
}
