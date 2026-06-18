use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{
    auth_action_button, big_code_lines, dots_frame, format_countdown, hints_list, key_hint,
    spinner_frame,
};
use crate::theme::{
    COLOR_ACCENT, COLOR_BG_PANEL, COLOR_CODE_BG, COLOR_DISABLED, COLOR_ERROR, COLOR_ERROR_BRIGHT,
    COLOR_LINK, COLOR_MUTED, COLOR_PRIMARY, COLOR_PRIMARY_BRIGHT, COLOR_SUCCESS_BRIGHT,
    COLOR_WARNING_BRIGHT,
};
use git_vacuum_app::state::{AuthErrorCategory, AuthMethodChoice, AuthPhase, AuthScreenState};

/// Top-level dispatcher. Matches the design doc §7 phases.
pub fn render_auth(f: &mut Frame, area: Rect, state: &AuthScreenState, tick: u64) {
    match state.phase {
        AuthPhase::MethodPicker => render_method_picker(f, area, state, tick),
        AuthPhase::PatInput => render_pat_input(f, area, state, tick),
        AuthPhase::Validating => render_validating(f, area, state, tick),
        AuthPhase::DeviceActivation => render_device_activation(f, area, state, tick),
        AuthPhase::AuthFailed => render_auth_failed(f, area, state, tick),
    }
}

/// Adaptive centered panel — 70 cols wide, max 22 rows tall. Shrinks
/// gracefully on narrow terminals. Renders a Clear over the area and
/// returns (outer, inner) rects for the caller to fill.
fn auth_panel(f: &mut Frame, area: Rect) -> (Rect, Rect) {
    let panel_w = 70u16.min(area.width.saturating_sub(2));
    let max_h = area.height.saturating_sub(2);
    let panel_h = max_h.clamp(10, 22);
    let outer = Rect {
        x: area.x + (area.width.saturating_sub(panel_w)) / 2,
        y: area.y + (area.height.saturating_sub(panel_h)) / 2,
        width: panel_w,
        height: panel_h,
    };
    let inner = Rect {
        x: outer.x + 1,
        y: outer.y + 1,
        width: outer.width.saturating_sub(2),
        height: outer.height.saturating_sub(2),
    };
    f.render_widget(ratatui::widgets::Clear, outer);
    (outer, inner)
}

fn panel_border_style(state: &AuthScreenState) -> Style {
    if state.phase == AuthPhase::AuthFailed {
        Style::default().fg(COLOR_ERROR_BRIGHT)
    } else if state.phase == AuthPhase::Validating || state.phase == AuthPhase::DeviceActivation {
        Style::default().fg(COLOR_ACCENT)
    } else {
        Style::default().fg(COLOR_PRIMARY_BRIGHT)
    }
}

fn panel_title(state: &AuthScreenState) -> &'static str {
    match state.phase {
        AuthPhase::MethodPicker => " Welcome to Git-Vacuum ",
        AuthPhase::PatInput => " Personal Access Token ",
        AuthPhase::Validating => " Authenticating... ",
        AuthPhase::DeviceActivation => " Device Activation ",
        AuthPhase::AuthFailed => " Authentication Failed ",
    }
}

/// Brand line: ⬢ git-vacuum + tagline + spinner.
fn render_brand(f: &mut Frame, area: Rect, tick: u64) {
    let brand = Paragraph::new(Line::from(vec![
        Span::styled(
            " ⬢ git-vacuum ",
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  GitHub Backup & Sync", Style::default().fg(COLOR_MUTED)),
        Span::raw("    "),
        Span::styled(spinner_frame(tick), Style::default().fg(COLOR_ACCENT)),
    ]))
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(brand, area);
}

/// ──────────────────────────────────────────────────────────────────────────
/// §7a — Method picker
/// ──────────────────────────────────────────────────────────────────────────
fn render_method_picker(f: &mut Frame, area: Rect, state: &AuthScreenState, tick: u64) {
    let (outer, inner) = auth_panel(f, area);

    // Layout inside the panel:
    //   [brand] [subtitle] [method list × 3] [scope note] [key bar]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // brand
            Constraint::Length(1), // spacer
            Constraint::Length(1), // subtitle
            Constraint::Length(1), // spacer
            Constraint::Length(5), // 3 method rows + padding
            Constraint::Length(1), // spacer
            Constraint::Length(2), // scope note
            Constraint::Min(0),    // filler
            Constraint::Length(1), // key bar
        ])
        .split(inner);

    render_brand(f, chunks[0], tick);

    let subtitle = Paragraph::new(Line::from(Span::styled(
        "  Connect your GitHub account to discover and sync your repositories.",
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(subtitle, chunks[2]);

    // Method rows. The cursor is `state.method_cursor` (0..=2).
    let oauth_disabled = state.oauth_client_id.as_deref().unwrap_or("").is_empty();
    let methods: [(AuthMethodChoice, &str, &str, bool); 3] = [
        (
            AuthMethodChoice::Pat,
            "Personal Access Token",
            "Classic token with repo + read:org scopes",
            false,
        ),
        (
            AuthMethodChoice::OAuth,
            "OAuth Device Flow",
            "Browser-based, no token to copy",
            oauth_disabled,
        ),
        (
            AuthMethodChoice::GhCli,
            "gh CLI Token",
            "Use your existing 'gh auth token'",
            true, // always disabled in this iteration
        ),
    ];

    let mut method_lines: Vec<Line> = Vec::new();
    for (i, (method, label, desc, disabled)) in methods.iter().enumerate() {
        let is_cursor = i as u8 == state.method_cursor;
        let pointer = if is_cursor { "▸" } else { " " };
        let tag = match method {
            AuthMethodChoice::Pat => "ENTER",
            AuthMethodChoice::OAuth if *disabled => "NO CLIENT_ID",
            AuthMethodChoice::OAuth => "ENTER",
            AuthMethodChoice::GhCli => "SOON",
        };
        let base_style = if *disabled {
            Style::default().fg(COLOR_DISABLED)
        } else if is_cursor {
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .bg(COLOR_BG_PANEL)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_PRIMARY_BRIGHT)
        };
        let desc_style = if *disabled {
            Style::default().fg(COLOR_DISABLED)
        } else {
            Style::default().fg(COLOR_MUTED)
        };
        let tag_style = if *disabled {
            Style::default().fg(COLOR_DISABLED)
        } else if is_cursor {
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_MUTED)
        };
        method_lines.push(Line::from(vec![
            Span::styled(format!("  {pointer}  "), base_style),
            Span::styled(format!("{label:<24}"), base_style),
            Span::styled(format!("  {desc:<40}"), desc_style),
            Span::styled(format!("  {tag}"), tag_style),
        ]));
    }
    let methods_widget = Paragraph::new(method_lines);
    f.render_widget(methods_widget, chunks[4]);

    // Scope note.
    let scope = Paragraph::new(Line::from(vec![
        Span::styled("  Token needs: ", Style::default().fg(COLOR_MUTED)),
        Span::styled("repo", Style::default().fg(COLOR_SUCCESS_BRIGHT)),
        Span::styled("  ", Style::default()),
        Span::styled("read:org", Style::default().fg(COLOR_SUCCESS_BRIGHT)),
        Span::styled("  ", Style::default()),
        Span::styled("user", Style::default().fg(COLOR_SUCCESS_BRIGHT)),
        Span::styled(
            "    (read-only is sufficient)",
            Style::default().fg(COLOR_MUTED),
        ),
    ]))
    .alignment(ratatui::layout::Alignment::Left);
    f.render_widget(scope, chunks[6]);

    // Key bar
    let kb = Line::from(vec![
        key_hint("↑↓", "method"),
        key_hint("Enter", "select"),
        key_hint("1-3", "jump"),
        key_hint("?", "help"),
        key_hint("q", "quit"),
    ]);
    f.render_widget(Paragraph::new(kb), chunks[8]);

    // Outer border
    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(panel_border_style(state))
        .title(Span::styled(
            panel_title(state),
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(border, outer);
}

/// ──────────────────────────────────────────────────────────────────────────
/// §7a — PAT input
/// ──────────────────────────────────────────────────────────────────────────
fn render_pat_input(f: &mut Frame, area: Rect, state: &AuthScreenState, tick: u64) {
    let (outer, inner) = auth_panel(f, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // brand
            Constraint::Length(1), // spacer
            Constraint::Length(2), // label
            Constraint::Length(3), // input box
            Constraint::Length(1), // spacer
            Constraint::Length(2), // status
            Constraint::Min(0),
            Constraint::Length(1), // key bar
        ])
        .split(inner);

    render_brand(f, chunks[0], tick);

    let label = Paragraph::new(Line::from(Span::styled(
        "  Paste your GitHub Personal Access Token:",
        Style::default().fg(COLOR_PRIMARY_BRIGHT),
    )));
    f.render_widget(label, chunks[2]);

    // Masked input with a blinking cursor.
    let masked: String = "•".repeat(state.token_input.chars().count());
    let cursor = if (tick / 4) % 2 == 0 { "▌" } else { " " };
    let input_text = format!("  {masked}{cursor} ");
    let input_para = Paragraph::new(Line::from(Span::styled(
        input_text,
        Style::default().fg(COLOR_PRIMARY_BRIGHT),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_PRIMARY_BRIGHT))
            .title(Span::styled(
                " Token ",
                Style::default().fg(COLOR_PRIMARY_BRIGHT),
            )),
    );
    f.render_widget(input_para, chunks[3]);

    let len = state.token_input.chars().count();
    let status = if state.token_input.is_empty() {
        Line::from(vec![
            Span::styled("  Press Enter to submit", Style::default().fg(COLOR_MUTED)),
            Span::styled("    ·    ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                "Press 'o' to sign in with browser",
                Style::default().fg(COLOR_PRIMARY_BRIGHT),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("  {len} chars"),
                Style::default().fg(COLOR_SUCCESS_BRIGHT),
            ),
            Span::styled(
                "   ·   Press Enter to submit, Backspace to delete, 'o' for OAuth",
                Style::default().fg(COLOR_MUTED),
            ),
        ])
    };
    f.render_widget(Paragraph::new(status), chunks[5]);

    let kb = Line::from(vec![
        key_hint("Enter", "submit"),
        key_hint("Bksp", "delete"),
        key_hint("o", "OAuth"),
        key_hint("Esc", "back"),
        key_hint("q", "quit"),
    ]);
    f.render_widget(Paragraph::new(kb), chunks[7]);

    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(panel_border_style(state))
        .title(Span::styled(
            panel_title(state),
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(border, outer);
}

/// ──────────────────────────────────────────────────────────────────────────
/// §7c — Validating
/// ──────────────────────────────────────────────────────────────────────────
fn render_validating(f: &mut Frame, area: Rect, state: &AuthScreenState, tick: u64) {
    let (outer, inner) = auth_panel(f, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // brand
            Constraint::Length(2), // spacer
            Constraint::Length(2), // headline
            Constraint::Length(1), // spacer
            Constraint::Length(4), // step list
            Constraint::Length(1), // spacer
            Constraint::Length(2), // hint
            Constraint::Min(0),
            Constraint::Length(1), // key bar
        ])
        .split(inner);

    render_brand(f, chunks[0], tick);

    let headline = Paragraph::new(Line::from(Span::styled(
        format!(
            "  {}  Verifying credentials with GitHub{}",
            spinner_frame(tick),
            dots_frame(tick)
        ),
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD),
    )));
    f.render_widget(headline, chunks[2]);

    // Step list — animated dots, never completes (the real "done" comes
    // from the success/failure event and the phase changes).
    let step1 = Line::from(vec![
        Span::styled("  ⠿  ", Style::default().fg(COLOR_ACCENT)),
        Span::styled(
            "Checking token scopes",
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ),
        Span::styled(dots_frame(tick), Style::default().fg(COLOR_PRIMARY_BRIGHT)),
    ]);
    let step2 = Line::from(vec![
        Span::styled("  ⠿  ", Style::default().fg(COLOR_ACCENT)),
        Span::styled(
            "Fetching user profile",
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ),
        Span::styled(
            dots_frame(tick + 2),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ),
    ]);
    let steps = Paragraph::new(vec![step1, Line::from(""), step2]);
    f.render_widget(steps, chunks[4]);

    let hint = Paragraph::new(Line::from(Span::styled(
        "  Do not close this window.",
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(hint, chunks[6]);

    let kb = Line::from(vec![key_hint("Esc", "cancel")]);
    f.render_widget(Paragraph::new(kb), chunks[8]);

    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(panel_border_style(state))
        .title(Span::styled(
            panel_title(state),
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(border, outer);
}

/// ──────────────────────────────────────────────────────────────────────────
/// §7b — Device activation
/// ──────────────────────────────────────────────────────────────────────────
fn render_device_activation(f: &mut Frame, area: Rect, state: &AuthScreenState, tick: u64) {
    let (outer, inner) = auth_panel(f, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // brand
            Constraint::Length(2), // spacer
            Constraint::Length(1), // step label
            Constraint::Length(3), // code box
            Constraint::Length(1), // spacer
            Constraint::Length(2), // status / countdown
            Constraint::Length(1), // spacer
            Constraint::Length(2), // hint
            Constraint::Min(0),
            Constraint::Length(1), // key bar
        ])
        .split(inner);

    render_brand(f, chunks[0], tick);

    // ── Enter this code ─────────────────────────────────────────────
    let step = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default().fg(COLOR_MUTED)),
        Span::styled(
            "Enter this code on GitHub:",
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ),
    ]));
    f.render_widget(step, chunks[2]);

    if let Some(oauth) = state.oauth.as_ref() {
        let code_inner_width = chunks[3].width.saturating_sub(4) as usize;
        let code_lines = big_code_lines(&oauth.user_code, code_inner_width);
        let code_box = Paragraph::new(code_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(COLOR_ACCENT))
                    .title(Span::styled(
                        " Code ",
                        Style::default().fg(COLOR_PRIMARY_BRIGHT),
                    )),
            )
            .style(Style::default().bg(COLOR_CODE_BG));
        f.render_widget(code_box, chunks[3]);
    } else {
        let placeholder = Paragraph::new(Line::from(Span::styled(
            format!("  {} Waiting for code from GitHub...", spinner_frame(tick)),
            Style::default().fg(COLOR_MUTED),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_PRIMARY_BRIGHT))
                .title(Span::styled(
                    " Code ",
                    Style::default().fg(COLOR_PRIMARY_BRIGHT),
                )),
        );
        f.render_widget(placeholder, chunks[3]);
    }

    // ── Status / countdown ─────────────────────────────────────────
    let status_line = if let Some(oauth) = state.oauth.as_ref() {
        let remaining = oauth
            .expires_at
            .saturating_duration_since(std::time::Instant::now());
        let countdown = format_countdown(remaining);
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!(
                    "{} Waiting for authorization{}",
                    spinner_frame(tick),
                    dots_frame(tick)
                ),
                Style::default().fg(COLOR_ACCENT),
            ),
            Span::styled("    (timeout in ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                countdown,
                Style::default()
                    .fg(COLOR_WARNING_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(")", Style::default().fg(COLOR_MUTED)),
        ])
    } else {
        Line::from(Span::styled(
            "  (No client_id configured — press Esc to go back.)",
            Style::default().fg(COLOR_WARNING_BRIGHT),
        ))
    };
    f.render_widget(Paragraph::new(status_line), chunks[5]);

    let hint = Paragraph::new(Line::from(Span::styled(
        "  The app continues automatically once you authorize.",
        Style::default().fg(COLOR_MUTED),
    )));
    f.render_widget(hint, chunks[7]);

    let kb = if state.show_url_prompt {
        Line::from(vec![
            key_hint("Enter", "open browser"),
            key_hint("Esc", "skip"),
        ])
    } else {
        Line::from(vec![
            key_hint("Enter", "open URL"),
            key_hint("o", "open URL"),
            key_hint("c", "copy code"),
            key_hint("Esc", "back"),
        ])
    };
    f.render_widget(Paragraph::new(kb), chunks[9]);

    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(panel_border_style(state))
        .title(Span::styled(
            panel_title(state),
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(border, outer);

    // ── Prompt to open browser URL ─────────────────────────────────
    if state.show_url_prompt {
        if let Some(oauth) = state.oauth.as_ref() {
            render_url_prompt(f, outer, &oauth.verification_uri);
        }
    }
}

fn render_url_prompt(f: &mut Frame, area: Rect, url: &str) {
    let popup = centered_rect(50, 30, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_ACCENT))
        .title(Span::styled(
            " Open browser? ",
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let text = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Open GitHub device activation page",
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "in your default browser?",
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            url,
            Style::default()
                .fg(COLOR_LINK)
                .add_modifier(Modifier::UNDERLINED),
        )]),
    ]);
    f.render_widget(text, inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// ──────────────────────────────────────────────────────────────────────────
/// §7e — Auth failed
/// ──────────────────────────────────────────────────────────────────────────
fn render_auth_failed(f: &mut Frame, area: Rect, state: &AuthScreenState, tick: u64) {
    let (outer, inner) = auth_panel(f, area);

    let err = state.error.as_ref();
    let inner_w = inner.width.saturating_sub(4) as usize;

    // Real focus cursor from state — 0 = "Try Again", 1 = "Pick a
    // different method". The user moves between them with Tab / arrows
    // (handled in main.rs via AuthFailedFocusMoved).
    let focus_try_again = state.failed_focus == 0;

    // Estimate layout height dynamically.
    let detail_h = if let Some(e) = err {
        (e.detail.chars().count() / inner_w.max(1)).max(1) + 1
    } else {
        1
    };
    let hints_h = err.map(|e| e.hints.len() as u16 * 2).unwrap_or(0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                      // brand
            Constraint::Length(1),                      // spacer
            Constraint::Length(2),                      // ✗ + headline
            Constraint::Length(1),                      // spacer
            Constraint::Length(detail_h.min(6) as u16), // detail
            Constraint::Length(1),                      // spacer
            Constraint::Length(hints_h.min(8)),         // hints
            Constraint::Length(1),                      // spacer
            Constraint::Length(3),                      // buttons row
            Constraint::Min(0),
            Constraint::Length(1), // key bar
        ])
        .split(inner);

    render_brand(f, chunks[0], tick);

    // ✗ + headline
    let headline = Paragraph::new(Line::from(vec![
        Span::styled(
            "  ✗  ",
            Style::default()
                .fg(COLOR_ERROR_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            err.map(|e| e.headline.as_str())
                .unwrap_or("Authentication failed"),
            Style::default()
                .fg(COLOR_ERROR_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    f.render_widget(headline, chunks[2]);

    // Detail block
    if let Some(e) = err {
        let detail_color = match e.category {
            AuthErrorCategory::Network => COLOR_WARNING_BRIGHT,
            AuthErrorCategory::OAuthConfig => COLOR_WARNING_BRIGHT,
            AuthErrorCategory::AccessDenied => COLOR_WARNING_BRIGHT,
            _ => COLOR_PRIMARY_BRIGHT,
        };
        let detail = Paragraph::new(Line::from(Span::styled(
            format!("  {}", e.detail),
            Style::default().fg(detail_color),
        )))
        .wrap(Wrap { trim: false });
        f.render_widget(detail, chunks[4]);
    }

    // Hints
    if let Some(e) = err {
        if !e.hints.is_empty() {
            let hint_lines = hints_list(&e.hints, inner_w);
            f.render_widget(Paragraph::new(hint_lines), chunks[6]);
        }
    }

    // Two action buttons. We render them in a horizontal row.
    let button_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[8]);
    f.render_widget(
        Paragraph::new(auth_action_button("Try Again", focus_try_again)),
        button_chunks[0],
    );
    f.render_widget(
        Paragraph::new(auth_action_button(
            "Pick a different method",
            !focus_try_again,
        )),
        button_chunks[1],
    );

    let kb = Line::from(vec![
        key_hint("Tab", "switch"),
        key_hint("Enter", "activate"),
        key_hint("Esc", "back"),
    ]);
    f.render_widget(Paragraph::new(kb), chunks[10]);

    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(panel_border_style(state))
        .title(Span::styled(
            panel_title(state),
            Style::default()
                .fg(COLOR_ERROR_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(border, outer);

    // Silence unused warnings for colors/types we keep imported for
    // future polish.
    let _ = Wrap { trim: false };
    let _ = COLOR_PRIMARY;
    let _ = COLOR_ERROR;
    let _ = COLOR_BG_PANEL;
}
