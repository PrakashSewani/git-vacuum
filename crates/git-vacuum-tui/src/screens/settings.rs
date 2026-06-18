use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use git_vacuum_app::tabs::SettingsTabState;
use git_vacuum_core::SettingsFieldKind;
use crate::theme::{COLOR_ACCENT, COLOR_MUTED, COLOR_PRIMARY, COLOR_PRIMARY_BRIGHT, COLOR_SUCCESS_BRIGHT, COLOR_WARNING_BRIGHT};

pub fn render_settings(f: &mut Frame, area: Rect, state: &SettingsTabState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(0)])
        .margin(1)
        .split(area);

    // Sidebar: category list
    let cats = git_vacuum_core::SettingsCategory::all();
    let items: Vec<ListItem> = cats.iter().map(|c| {
        let style = if *c == state.selected_category {
            Style::default().bg(ratatui::style::Color::DarkGray).fg(COLOR_PRIMARY_BRIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_MUTED)
        };
        let marker = if *c == state.selected_category { "▸ " } else { "  " };
        ListItem::new(Line::from(Span::styled(
            format!("{}{}", marker, c.label()),
            style,
        )))
    }).collect();
    let sidebar = List::new(items).block(Block::default().borders(Borders::ALL).title(" Categories "));
    f.render_widget(sidebar, chunks[0]);

    // Right: settings body
    let body_block = Block::default().borders(Borders::ALL).title(" Settings ");
    let inner = body_block.inner(chunks[1]);
    f.render_widget(body_block, chunks[1]);

    if state.fields.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            " No settings in this category.",
            Style::default().fg(COLOR_MUTED),
        )));
        f.render_widget(p, inner);
        return;
    }

    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),  // fields
            Constraint::Length(2), // help
            Constraint::Length(1), // status
        ])
        .split(inner);

    let lines: Vec<Line> = state.fields.iter().enumerate().map(|(i, field)| {
        let is_selected = i == state.selected_field;
        let is_editing = state.editing_field == Some(i);
        let pointer = if is_selected { "▸ " } else { "  " };
        let base_style = if is_selected {
            Style::default().bg(ratatui::style::Color::DarkGray).fg(COLOR_PRIMARY_BRIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_PRIMARY)
        };

        let value_display = if is_editing {
            Span::styled(
                format!("{}▌", state.draft_value),
                Style::default().fg(COLOR_WARNING_BRIGHT).add_modifier(Modifier::BOLD),
            )
        } else {
            match &field.kind {
                SettingsFieldKind::Boolean => {
                    let on = field.value == "true";
                    let icon = if on { "[x]" } else { "[ ]" };
                    let color = if on { COLOR_SUCCESS_BRIGHT } else { COLOR_MUTED };
                    Span::styled(icon.to_string(), Style::default().fg(color))
                }
                SettingsFieldKind::Dropdown { options: _ } => {
                    Span::styled(
                        format!("▾ {}", field.value),
                        Style::default().fg(COLOR_PRIMARY_BRIGHT),
                    )
                }
                _ => Span::styled(field.value.clone(), Style::default().fg(COLOR_PRIMARY_BRIGHT)),
            }
        };

        Line::from(vec![
            Span::styled(pointer, base_style),
            Span::styled(format!("{:<20}", field.label), base_style),
            Span::raw("  "),
            value_display,
        ])
    }).collect();
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, body_chunks[0]);

    // Help
    let help_text = if let Some(field) = state.fields.get(state.selected_field) {
        if let Some(help) = &field.help {
            help.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let help = Paragraph::new(Line::from(Span::styled(
        help_text,
        Style::default().fg(COLOR_MUTED),
    )))
    .wrap(Wrap { trim: false })
    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(COLOR_MUTED)));
    f.render_widget(help, body_chunks[1]);

    // Status
    let status = if state.editing_field.is_some() {
        Line::from(Span::styled(
            " Editing — Enter: save  Esc: discard",
            Style::default().fg(COLOR_ACCENT),
        ))
    } else if state.has_unsaved_changes {
        Line::from(Span::styled(
            " * unsaved changes — Ctrl+S: save all",
            Style::default().fg(COLOR_WARNING_BRIGHT),
        ))
    } else {
        Line::from(Span::styled(
            " Tab: category  ↑↓: navigate  Enter: toggle/edit  Ctrl+S: save",
            Style::default().fg(COLOR_MUTED),
        ))
    };
    f.render_widget(Paragraph::new(status), body_chunks[2]);
}
