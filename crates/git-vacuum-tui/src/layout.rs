use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Outer chrome: title bar (3 rows now: brand+spinner, stats, breathing border)
/// + tab bar (1) + activity banner (1 if loading) + main content + key bar (1) = 6-7 rows.
pub fn shell_layout(area: Rect, show_activity_banner: bool) -> Vec<Rect> {
    let mut constraints = vec![
        Constraint::Length(3), // title bar
        Constraint::Length(1), // tab bar
    ];
    if show_activity_banner {
        constraints.push(Constraint::Length(1));
    }
    constraints.extend([
        Constraint::Min(0),    // main content
        Constraint::Length(1), // key bar
    ]);
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
        .to_vec()
}
