use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Outer chrome: title bar (2) + tab bar (1) + main content + key bar (1) = 5 rows total.
pub fn shell_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // title bar
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // main content
            Constraint::Length(1), // key bar
        ])
        .split(area)
        .to_vec()
}
