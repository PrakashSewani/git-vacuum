use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::tabs::SettingsTabState;
use crate::theme::{COLOR_MUTED, COLOR_PRIMARY};

pub fn render_settings(f: &mut Frame, area: Rect, state: &SettingsTabState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(0)])
        .margin(1)
        .split(area);

    // Sidebar: category list
    let cats = [
        git_vacuum_core::SettingsCategory::General,
        git_vacuum_core::SettingsCategory::Clone,
        git_vacuum_core::SettingsCategory::Sync,
        git_vacuum_core::SettingsCategory::GitHub,
        git_vacuum_core::SettingsCategory::Advanced,
    ];
    let items: Vec<ListItem> = cats.iter().enumerate().map(|(i, c)| {
        let style = if *c == state.selected_category {
            Style::default().bg(ratatui::style::Color::DarkGray).fg(COLOR_PRIMARY).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(Line::from(format!(" {}", c.label()))).style(style)
    }).collect();
    let sidebar = List::new(items).block(Block::default().borders(Borders::ALL).title(" Categories "));
    f.render_widget(sidebar, chunks[0]);

    // Right: placeholder for now (the form is in app::tabs::SettingsTabState.fields)
    let body = if state.fields.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            " Settings are loaded on first run. (stub — see app::tabs::SettingsTabState)",
            Style::default().fg(COLOR_MUTED),
        )))
    } else {
        let lines: Vec<Line> = state.fields.iter().map(|f| {
            Line::from(format!("  {}: {}", f.label, f.value))
        }).collect();
        Paragraph::new(lines)
    };
    let body = body.block(Block::default().borders(Borders::ALL).title(" Settings "))
        .wrap(Wrap { trim: false });
    f.render_widget(body, chunks[1]);
}
