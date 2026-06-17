# git-vacuum

A keyboard-first terminal UI for backing up, browsing, and synchronizing your GitHub repositories locally. Local-only — no server, no cloud, no telemetry. Your token stays in your OS keyring.

## Quickstart

### Prerequisites

- **Rust 1.85+** (install via [rustup](https://rustup.rs))
- **Windows:** Visual Studio Build Tools with the C++ workload (for the bundled `libgit2`)
- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Linux:** `cmake`, `pkg-config`, and a C compiler (`gcc`/`clang`). For the OS keyring, run inside a desktop session (GNOME Keyring, KWallet, etc.) — headless servers without Secret Service are not supported in MVP; use `--token <pat>` for one-off runs.
- **A GitHub Personal Access Token** with `repo` and `read:org` scopes. Create one at <https://github.com/settings/tokens>.

### Build & run

```bash
git clone https://github.com/PrakashSewani/git-vacuum
cd git-vacuum
cargo run --release -p git-vacuum
```

On first launch you'll see the auth screen. Paste your token (it's masked). It's stored in the OS keyring (Windows Credential Manager, macOS Keychain, or Secret Service on Linux) — never on disk in plaintext.

### Non-interactive mode (headless / CI)

```bash
git-vacuum --token ghp_xxx --sync
```

This runs the full discovery + sync without a TUI. Useful for cron jobs.

## Architecture

- **9-crate Cargo workspace** with hexagonal boundaries (see `docs/design/git-vacuum-workspace-structure.md`)
- **No backend.** The TUI talks directly to GitHub. See `docs/design/no-backend-rationale.md`.
- **Token storage:** OS keyring only. See `docs/design/git-vacuum-github-integration.md` §2.6 for the security contract.

## Security

- The token is **never** in SQLite, **never** in logs, **never** in error messages, **never** on disk in plaintext.
- Audit checklist in `docs/design/git-vacuum-github-integration.md` §2.6.5.

## License

MIT — see `LICENSE`.
