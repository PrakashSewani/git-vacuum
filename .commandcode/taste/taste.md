# workflow
- For early project stages, follow a design-first sequence: product specification → UX design → architecture → implementation. Defer code until design and architecture are locked. Confidence: 0.75
- For design and planning phases: provide architecture and design documentation without implementation code. Write design documents in markdown rather than producing code. Confidence: 0.75

# tech-stack
- For TUI applications: use Rust, Ratatui, Tokio, Octocrab (GitHub API), SQLite, and git2-rs. Confidence: 0.65

# ux
- For TUI applications in this project, adopt keyboard-first interaction inspired by lazygit and k9s. Every action must have a visible keyboard shortcut and be discoverable from the command bar. Confidence: 0.70

# logging
- Avoid adding verbose log::debug!/log::info! calls throughout the codebase; user finds them annoying. Keep logging minimal or remove it. Confidence: 0.80

