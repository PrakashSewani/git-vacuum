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
    // On Windows Conhost, enabling bracketed paste (\x1b[?2004h) is known to
    // cause the terminal to enter a broken state where the next read can panic.
    // We only enable it on Unix-like systems where it's well-supported.
    // Paste still works on Windows via the explicit Ctrl+V handler that
    // reads the clipboard with arboard.
    #[cfg(not(windows))]
    {
        use crossterm::event::{EnableBracketedPaste};
        let _ = execute!(out, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste);
    }
    #[cfg(windows)]
    {
        let _ = execute!(out, EnterAlternateScreen, EnableMouseCapture);
    }
    let backend = CrosstermBackend::new(out);
    Terminal::new(backend)
}

pub fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    let mut out = std::io::stdout();
    #[cfg(not(windows))]
    {
        use crossterm::event::{DisableBracketedPaste};
        let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture, DisableBracketedPaste);
    }
    #[cfg(windows)]
    {
        let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture);
    }
    Ok(())
}
