use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{dots_frame, progress_bar, spinner_frame};
use crate::theme::{
    COLOR_ACCENT, COLOR_BG_BANNER, COLOR_BG_PANEL, COLOR_ERROR, COLOR_MUTED, COLOR_PRIMARY,
    COLOR_PRIMARY_BRIGHT, COLOR_SUCCESS_BRIGHT, COLOR_WARNING_BRIGHT,
};
use git_vacuum_app::state::WelcomePhase;
use git_vacuum_core::UserInfo;

const LOGO: &[&str] = &[
    "  ⬢   ⬡   ⬢   ⬡   ⬢",
    "  ╔═╗╦╔═╗╔═╗ ╦ ╦╔═╗╦ ╦╔═╗",
    "  ║ ╦║║ ╦║ ╦ ║ ║║ ║║ ║╚═╗",
    "  ╚═╝╩╚═╝╚═╝ ╚═╝╚═╝╚═╝╚═╝",
];

pub fn render_welcome(
    f: &mut Frame,
    area: Rect,
    user: &UserInfo,
    repos_count: Option<usize>,
    phase: WelcomePhase,
    tick: u64,
) {
    // Center the welcome panel
    let panel_h: u16 = 24;
    let panel_w: u16 = 80;
    let panel = Rect {
        x: area.x + area.width.saturating_sub(panel_w) / 2,
        y: area.y + area.height.saturating_sub(panel_h) / 2,
        width: panel_w.min(area.width),
        height: panel_h.min(area.height),
    };
    f.render_widget(ratatui::widgets::Clear, panel);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // logo
            Constraint::Length(2), // greeting
            Constraint::Length(2), // spinner + status
            Constraint::Length(3), // progress (Summary phase only)
            Constraint::Length(2), // hint
            Constraint::Length(1), // prompt
        ])
        .margin(2)
        .split(panel);

    // Logo
    let logo_lines: Vec<Line> = LOGO
        .iter()
        .map(|l| {
            Line::from(Span::styled(
                *l,
                Style::default()
                    .fg(COLOR_PRIMARY_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    let logo = Paragraph::new(logo_lines)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(logo, chunks[0]);

    // Greeting
    let greeting_text = match phase {
        WelcomePhase::Greeting => format!("Welcome back, {}", user.login),
        WelcomePhase::Summary => format!("Hello, {}", user.login),
        WelcomePhase::Ready => "Ready.".to_string(),
    };
    let greeting = Paragraph::new(Line::from(Span::styled(
        greeting_text,
        Style::default()
            .fg(COLOR_SUCCESS_BRIGHT)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().borders(Borders::NONE));
    f.render_widget(greeting, chunks[1]);

    // Spinner + status
    let spinner_text = match phase {
        WelcomePhase::Greeting => format!(
            "{} Connecting to GitHub{}",
            spinner_frame(tick),
            dots_frame(tick)
        ),
        WelcomePhase::Summary => format!(
            "{} Loading your repositories{}",
            spinner_frame(tick),
            dots_frame(tick)
        ),
        WelcomePhase::Ready => format!("{} Setup complete", spinner_frame(tick)),
    };
    let spinner = Paragraph::new(Line::from(Span::styled(
        spinner_text,
        Style::default().fg(COLOR_ACCENT),
    )))
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().borders(Borders::NONE));
    f.render_widget(spinner, chunks[2]);

    // Progress bar (Summary and Ready phases)
    if matches!(phase, WelcomePhase::Summary | WelcomePhase::Ready) {
        let percent = match phase {
            WelcomePhase::Summary => 0.6 + (tick as f32 / 100.0).sin() * 0.1,
            WelcomePhase::Ready => 1.0,
            _ => 0.0,
        };
        let bar = progress_bar(60, percent, tick);
        let label = match (repos_count, phase) {
            (Some(n), WelcomePhase::Summary) => format!("{} repositories discovered", n),
            (Some(n), WelcomePhase::Ready) => format!("✓ {} repositories", n),
            (None, _) => "Discovering...".to_string(),
            _ => "✓".to_string(),
        };
        let body = vec![
            Line::from(Span::styled(bar, Style::default().fg(COLOR_PRIMARY_BRIGHT))),
            Line::from(Span::styled(label, Style::default().fg(COLOR_MUTED))),
        ];
        let p = Paragraph::new(body).alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, chunks[3]);
    }

    // Hint
    let hint = Paragraph::new(Line::from(Span::styled(
        "Tab to switch panes · 1-5 to jump · : for command palette · ? for help",
        Style::default().fg(COLOR_MUTED),
    )))
    .alignment(ratatui::layout::Alignment::Center)
    .wrap(Wrap { trim: false });
    f.render_widget(hint, chunks[4]);

    // Prompt (Ready phase only)
    let prompt = if matches!(phase, WelcomePhase::Ready) {
        Paragraph::new(Line::from(Span::styled(
            "[ press any key to continue ]",
            Style::default()
                .fg(COLOR_WARNING_BRIGHT)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(ratatui::layout::Alignment::Center)
    } else {
        Paragraph::new("")
    };
    f.render_widget(prompt, chunks[5]);

    // Border
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_PRIMARY))
        .title(Span::styled(
            " git-vacuum ",
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(outer, panel);

    // Suppress unused warnings
    let _ = COLOR_BG_BANNER;
    let _ = COLOR_BG_PANEL;
    let _ = COLOR_ERROR;
}
