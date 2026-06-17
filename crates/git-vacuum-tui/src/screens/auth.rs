use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::state::AuthMode;
use crate::theme::{COLOR_ERROR, COLOR_MUTED, COLOR_PRIMARY};

pub fn render_auth(
    f: &mut Frame,
    area: Rect,
    mode: AuthMode,
    token_input: &str,
    error: Option<&str>,
    loading: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    // Header
    let header = Paragraph::new(vec![
        Line::from(Span::styled(" git-vacuum ", Style::default().fg(COLOR_PRIMARY).add_modifier(Modifier::BOLD))),
        Line::from(" Authenticate with GitHub to discover and back up your repositories."),
    ])
    .block(Block::default().borders(Borders::NONE));
    f.render_widget(header, chunks[0]);

    // Token input panel
    let masked: String = "*".repeat(token_input.chars().count());
    let input_text = match mode {
        AuthMode::Pat => vec![
            Line::from(""),
            Line::from(" Personal Access Token (recommended):"),
            Line::from(Span::styled(
                format!("  > {}_  ", if loading { "" } else { &masked }),
                Style::default().fg(COLOR_PRIMARY),
            )),
            Line::from(Span::styled("  (Token is stored in the OS keyring — never on disk.)", Style::default().fg(COLOR_MUTED))),
            Line::from(""),
            Line::from(Span::styled("  Press Enter to submit  ·  Esc to cancel", Style::default().fg(COLOR_MUTED))),
        ],
        AuthMode::OAuth => vec![
            Line::from(""),
            Line::from(" OAuth device flow is not yet implemented in MVP."),
            Line::from(" Use a Personal Access Token instead (recommended)."),
        ],
    };

    let mut body_lines = input_text;
    if let Some(err) = error {
        body_lines.push(Line::from(""));
        body_lines.push(Line::from(Span::styled(
            format!(" ⚠ {err}"),
            Style::default().fg(COLOR_ERROR),
        )));
    }

    let body = Paragraph::new(body_lines)
        .block(Block::default().borders(Borders::ALL).title(" Authenticate "))
        .wrap(Wrap { trim: false });
    f.render_widget(body, chunks[1]);

    // Footer
    let footer = Paragraph::new(Line::from(Span::styled(
        if loading { " Authenticating..." } else { " q: Quit  ?: Help" },
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(footer, chunks[2]);
}
