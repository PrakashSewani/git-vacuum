# workflow
- For early project stages, follow a design-first sequence: product specification → UX design → architecture → implementation. Defer code until design and architecture are locked. Confidence: 0.75
- For design and planning phases: provide architecture and design documentation without implementation code. Write design documents in markdown rather than producing code. Confidence: 0.75
- For large multi-component implementations, split into incremental milestone passes (e.g., 3-4 milestones) that each compile standalone, allowing review between passes rather than building all at once. Confidence: 0.70

# tech-stack
- For TUI applications: use Rust, Ratatui, Tokio, Octocrab (GitHub API), SQLite, and git2-rs. Confidence: 0.65

# ux
- For TUI applications in this project, adopt keyboard-first interaction inspired by lazygit and k9s. Every action must have a visible keyboard shortcut and be discoverable from the command bar. Confidence: 0.70

# logging
- Avoid adding verbose log::debug!/log::info! calls throughout the codebase; user finds them annoying. Keep logging minimal or remove it. Confidence: 0.80
- For Rust applications in this project, use env_logger with INFO level as the default (RUST_LOG=trace still works for debugging). Confidence: 0.65

# git-vacuum
- For git-vacuum "My Repos" source: include EVERY accessible repo (owner, collaborator, org_member) — do NOT filter out repos the user is only a collaborator on or has org membership in. User explicitly wants all of them. Confidence: 0.85
