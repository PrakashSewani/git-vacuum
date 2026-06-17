use std::io::{self, Stdout};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    Terminal::new(backend)
}

pub fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
