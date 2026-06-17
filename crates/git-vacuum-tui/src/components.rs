use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use git_vacuum_app::state::TabKind;
use git_vacuum_core::RepoEntry;
use crate::theme::{COLOR_BG_HIGHLIGHT, COLOR_HIGHLIGHT, COLOR_MUTED, COLOR_PRIMARY};

pub fn title_bar<'a>(user: Option<&git_vacuum_core::UserInfo>, stats: Option<&git_vacuum_core::DashboardStats>) -> Vec<Line<'a>> {
    let brand = Span::styled(" git-vacuum ", Style::default().fg(COLOR_PRIMARY).add_modifier(Modifier::BOLD));
    let user_span = match user {
        Some(u) => Span::raw(format!("  user: {}", u.login)),
        None => Span::styled("  (not authenticated)", Style::default().fg(COLOR_MUTED)),
    };
    let line1 = Line::from(vec![brand, user_span]);

    let line2 = match stats {
        Some(s) => Line::from(format!("  {} repos  ·  {} up-to-date  ·  {} behind  ·  {} errors",
            s.total_repos, s.up_to_date, s.behind, s.errors)),
        None => Line::from(Span::styled("  (loading stats...)", Style::default().fg(COLOR_MUTED))),
    };
    vec![line1, line2]
}

pub fn tab_bar(active: TabKind) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, t) in TabKind::all().iter().enumerate() {
        let label = t.label();
        let style = if *t == active {
            Style::default().fg(COLOR_HIGHLIGHT).add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(COLOR_MUTED)
        };
        spans.push(Span::styled(format!(" {} ", label), style));
        if i < TabKind::all().len() - 1 {
            spans.push(Span::raw("  "));
        }
    }
    Line::from(spans)
}

pub fn key_bar<'a>(bindings: &[(&str, &str)]) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    for (i, (key, desc)) in bindings.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(format!(" {}:", key), Style::default().fg(COLOR_PRIMARY).add_modifier(Modifier::BOLD)));
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
        Line::from(format!("{} {} {}  {}  {}", check, name_truncated, visibility, status, stars))
    } else {
        Line::from("")
    }
}

pub fn highlight_style() -> Style {
    Style::default().bg(COLOR_BG_HIGHLIGHT)
}
