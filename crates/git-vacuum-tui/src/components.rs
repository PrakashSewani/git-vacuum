use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::{
    COLOR_ACCENT, COLOR_BG_HIGHLIGHT, COLOR_BG_PANEL, COLOR_MUTED, COLOR_PRIMARY,
    COLOR_PRIMARY_BRIGHT, COLOR_SUCCESS_BRIGHT,
};
use git_vacuum_app::state::TabKind;
use git_vacuum_core::RepoEntry;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_frame(tick: u64) -> &'static str {
    SPINNER_FRAMES[(tick as usize) % SPINNER_FRAMES.len()]
}

pub fn dots_frame(tick: u64) -> &'static str {
    match (tick / 4) % 4 {
        0 => "   ",
        1 => ".  ",
        2 => ".. ",
        _ => "...",
    }
}

pub fn pulse_dot(tick: u64) -> &'static str {
    match (tick / 5) % 3 {
        0 => "·",
        1 => "•",
        _ => "●",
    }
}

pub fn breathing_char(tick: u64) -> char {
    let chars = ['─', '╌', '┈'];
    chars[(tick as usize / 2) % chars.len()]
}

/// Animated block progress bar, ~16 columns wide.
/// `percent` is clamped 0.0..=1.0. `tick` shifts the bright highlight horizontally.
pub fn progress_bar(width: usize, percent: f32, tick: u64) -> String {
    if width == 0 {
        return String::new();
    }
    let p = percent.clamp(0.0, 1.0);
    let filled = ((p * width as f32).round() as usize).min(width);
    let highlight_pos = ((tick as usize) / 2) % width.max(1);

    let mut s = String::with_capacity(width * 4);
    for i in 0..width {
        if i < filled {
            if i == filled.saturating_sub(1) || i == highlight_pos {
                s.push('█');
            } else if i % 2 == 0 {
                s.push('▓');
            } else {
                s.push('▒');
            }
        } else if i == filled && filled < width {
            s.push('▒');
        } else {
            s.push('░');
        }
    }
    s
}

pub fn title_bar<'a>(
    user: Option<&git_vacuum_core::UserInfo>,
    stats: Option<&git_vacuum_core::DashboardStats>,
    tick: u64,
) -> Vec<Line<'a>> {
    let brand = Span::styled(
        " ⬢ git-vacuum ",
        Style::default()
            .fg(COLOR_PRIMARY_BRIGHT)
            .add_modifier(Modifier::BOLD),
    );
    let user_span = match user {
        Some(u) => Span::styled(
            format!("  {} ", u.login),
            Style::default()
                .fg(COLOR_SUCCESS_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ),
        None => Span::styled("  (not authenticated) ", Style::default().fg(COLOR_MUTED)),
    };
    let spinner = Span::styled(
        format!(" {} ", spinner_frame(tick)),
        Style::default().fg(COLOR_ACCENT),
    );
    let line1 = Line::from(vec![brand, user_span, spinner]);

    let line2 = match stats {
        Some(s) => Line::from(vec![
            Span::styled("  ◉ ", Style::default().fg(COLOR_PRIMARY_BRIGHT)),
            Span::raw(format!(
                "{} repos  ·  ✓ {} synced  ·  ⚠ {} behind  ·  ✗ {} errors",
                s.total_repos, s.up_to_date, s.behind, s.errors
            )),
        ]),
        None => Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!(
                    "{} loading dashboard stats{}",
                    spinner_frame(tick),
                    dots_frame(tick)
                ),
                Style::default().fg(COLOR_MUTED),
            ),
        ]),
    };

    let border = breathing_char(tick);
    let line3 = Line::from(Span::styled(
        std::iter::repeat_n(border, 80).collect::<String>(),
        Style::default().fg(COLOR_PRIMARY),
    ));

    vec![line1, line2, line3]
}

pub fn tab_bar(active: TabKind, repos_loading: bool, tick: u64) -> Line<'static> {
    let mut spans = Vec::new();
    let number_glyphs = ["①", "②", "③", "④", "⑤"];
    for (i, t) in TabKind::all().iter().enumerate() {
        let label = t.label();
        let number = number_glyphs.get(i).copied().unwrap_or("•");
        let is_active = *t == active;

        // Pick a "pending" indicator: Explorer loads repos; Dashboard loads stats
        let pending = matches!(t, TabKind::Dashboard | TabKind::Explorer) && repos_loading;
        let pending_dot = if pending { pulse_dot(tick) } else { " " };

        let style = if is_active {
            Style::default()
                .fg(COLOR_PRIMARY_BRIGHT)
                .bg(COLOR_BG_PANEL)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_MUTED)
        };

        let label_styled = if is_active {
            format!(" {} {} {} ", number, label, pending_dot)
        } else {
            format!(" {} {}{} ", number, label, pending_dot)
        };
        spans.push(Span::styled(label_styled, style));
        if i < TabKind::all().len() - 1 {
            spans.push(Span::styled("  ", Style::default().fg(COLOR_MUTED)));
        }
    }
    Line::from(spans)
}

/// Returns the activity banner showing what the TUI is currently loading.
/// Returns `None` if nothing is loading.
pub fn activity_banner(
    loading: &git_vacuum_app::state::LoadingState,
    tick: u64,
) -> Option<Line<'static>> {
    if !loading.anything_pending() {
        return None;
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::styled(
        format!(" {} ", spinner_frame(tick)),
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD),
    ));
    if loading.repos {
        spans.push(Span::styled(
            "Discovering repositories from GitHub".to_string(),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ));
        spans.push(Span::styled(
            dots_frame(tick).to_string(),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ));
    }
    if loading.repos && loading.stats {
        spans.push(Span::styled("   ".to_string(), Style::default()));
        spans.push(Span::styled(
            "│".to_string(),
            Style::default().fg(COLOR_MUTED),
        ));
        spans.push(Span::styled("   ".to_string(), Style::default()));
    }
    if loading.stats {
        spans.push(Span::styled(
            "Computing dashboard stats".to_string(),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ));
        spans.push(Span::styled(
            dots_frame(tick + 5).to_string(),
            Style::default().fg(COLOR_PRIMARY_BRIGHT),
        ));
    }
    Some(Line::from(spans))
}

pub fn key_bar<'a>(bindings: &[(&str, &str)]) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    for (i, (key, desc)) in bindings.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!(" {}:", key),
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}", desc)));
    }
    Line::from(spans)
}

pub fn format_repo_row(repos: &[RepoEntry], idx: usize, max_width: usize) -> Line<'static> {
    if let Some(r) = repos.get(idx) {
        let check = if r.selected { "[x]" } else { "[ ]" };
        let visibility = match r.visibility {
            git_vacuum_core::RepoVisibility::Public => "pub",
            git_vacuum_core::RepoVisibility::Private => "priv",
            git_vacuum_core::RepoVisibility::Internal => "int",
        };
        let name = format!("{}/{}", r.owner_login, r.name);
        let name_truncated = if name.chars().count() > max_width.saturating_sub(20) {
            let mut s: String = name.chars().take(max_width.saturating_sub(21)).collect();
            s.push('…');
            s
        } else {
            name
        };
        let stars = format!("★{}", r.stars);
        let status = match r.clone_status {
            git_vacuum_core::CloneStatus::Cloned => "✓",
            git_vacuum_core::CloneStatus::NotCloned => " ",
            git_vacuum_core::CloneStatus::Stale => "⚠",
            git_vacuum_core::CloneStatus::Error => "✗",
        };
        Line::from(format!(
            "{} {} {}  {}  {}",
            check, name_truncated, visibility, status, stars
        ))
    } else {
        Line::from("")
    }
}

pub fn highlight_style() -> Style {
    Style::default().bg(COLOR_BG_HIGHLIGHT)
}

/// " ↵ :select" — a single key+description pair used in the key bar.
pub fn key_hint(key: &str, desc: &str) -> Span<'static> {
    Span::styled(
        format!(" {key}:{desc}"),
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )
}

/// "MM:SS" countdown for the device-flow expiry timer. Clamps to 0:00.
pub fn format_countdown(remaining: std::time::Duration) -> String {
    let total = remaining.as_secs();
    if total == 0 && remaining.subsec_nanos() == 0 {
        return "0:00".to_string();
    }
    let mins = total / 60;
    let secs = total % 60;
    format!("{mins}:{secs:02}")
}

/// Render a GitHub device-flow user code as a visually-emphasized block.
/// The code looks like "XK7F-2MPQ" — we pad it with full-block characters
/// on each side and use a high-contrast green on a tinted background.
///
/// On narrow terminals (<30 cols inner) we drop the padding and just show
/// the code, so the layout doesn't overflow.
pub fn big_code_lines(code: &str, inner_width: usize) -> Vec<Line<'static>> {
    use crate::theme::COLOR_CODE_BG;
    if inner_width < 30 {
        return vec![Line::from(Span::styled(
            format!("  {code}"),
            Style::default()
                .fg(COLOR_SUCCESS_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ))];
    }
    let pad = "▓".repeat(((inner_width.saturating_sub(code.chars().count())) / 2).max(1));
    let right_pad =
        "▓".repeat(inner_width.saturating_sub(pad.chars().count() + code.chars().count()));
    let styled_code = Span::styled(
        code.to_string(),
        Style::default()
            .fg(COLOR_SUCCESS_BRIGHT)
            .bg(COLOR_CODE_BG)
            .add_modifier(Modifier::BOLD),
    );
    let left_pad = Span::styled(pad, Style::default().fg(COLOR_CODE_BG));
    let right = Span::styled(right_pad, Style::default().fg(COLOR_CODE_BG));
    vec![Line::from(vec![left_pad, styled_code, right])]
}

/// Render a list of hint strings as a bulleted, wrapped list. Each hint
/// starts with a bullet ("• "). Long hints wrap at `width` columns.
pub fn hints_list(hints: &[String], width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for hint in hints {
        let prefix = "  • ";
        let content_width = width.saturating_sub(prefix.chars().count());
        // Naive char-based wrap. Hints are short enough that this is fine.
        let mut current = String::new();
        for word in hint.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word.chars().count() <= content_width {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(Line::from(Span::styled(
                    format!("{prefix}{current}"),
                    Style::default().fg(COLOR_MUTED),
                )));
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            out.push(Line::from(Span::styled(
                format!("{prefix}{current}"),
                Style::default().fg(COLOR_MUTED),
            )));
        }
    }
    out
}

/// Render a single focusable action button (used by the AuthFailed screen).
/// `focused` adds a reverse-video background and a `▸` pointer.
pub fn auth_action_button(label: &str, focused: bool) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(COLOR_BG_PANEL)
            .bg(COLOR_PRIMARY_BRIGHT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_PRIMARY_BRIGHT)
    };
    let pointer = if focused { "▸ " } else { "  " };
    Line::from(Span::styled(format!("  {pointer}[ {label} ]"), style))
}
