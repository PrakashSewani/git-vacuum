# git-vacuum

A keyboard-first terminal UI for backing up, browsing, and synchronizing your GitHub repositories locally. Local-only — no server, no cloud, no telemetry. Your token stays in your OS keyring.

## Features

- **Dashboard** — Overview stats, attention-needed repos (behind, ahead, dirty), size breakdown
- **Repo Explorer** — Browse all your repos (personal, org, starred), filter by topic, multi-select
- **Sync Center** — Clone and fetch repos with live progress, concurrent workers, and summary
- **Activity Log** — History of sync runs with per-repo outcomes
- **Settings** — Configure clone path, database path, concurrency, and OAuth client ID

Supports both **PAT (Personal Access Token)** and **OAuth device flow** (browser-based sign-in).

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

On first launch you'll see the auth screen. You can paste your PAT (masked input) or choose OAuth browser sign-in (requires `--oauth-client-id`). Tokens are stored in the OS keyring — never on disk in plaintext.

### CLI options

```
git-vacuum [OPTIONS]

Options:
  --token <TOKEN>            GitHub Personal Access Token (or set GITHUB_TOKEN env var)
  --sync                     Skip the TUI and just sync (headless mode)
  --oauth-client-id <ID>     GitHub OAuth App client_id for browser sign-in (or set GIT_VACUUM_OAUTH_CLIENT_ID)
  --db-path <PATH>           Path to the SQLite database (or set GIT_VACUUM_DB)
  --clone-path <PATH>        Where to clone repos (or set GIT_VACUUM_CLONE_PATH)
  --concurrency <N>          Concurrent clone/fetch operations [default: 8]
```

### Non-interactive mode (headless / CI)

```bash
git-vacuum --token ghp_xxx --sync
```

Runs the full discovery + sync without a TUI. Useful for cron jobs or CI pipelines.

### Environment variables

| Variable | Description |
|---|---|
| `GITHUB_TOKEN` | GitHub PAT (alternative to `--token`) |
| `GIT_VACUUM_OAUTH_CLIENT_ID` | OAuth App client_id (alternative to `--oauth-client-id`) |
| `GIT_VACUUM_DB` | SQLite database path (alternative to `--db-path`) |
| `GIT_VACUUM_CLONE_PATH` | Clone destination directory (alternative to `--clone-path`) |

## Architecture

9-crate Cargo workspace using hexagonal architecture (ports & adapters):

| Crate | Purpose |
|---|---|
| `git-vacuum-core` | Shared types, traits, events, errors — the kernel |
| `git-vacuum-db` | SQLite adapter (bundled rusqlite, WAL mode) |
| `git-vacuum-github` | GitHub API adapter (octocrab + reqwest for OAuth) |
| `git-vacuum-git` | Git operations adapter (git2-rs, vendored libgit2) |
| `git-vacuum-keyring` | OS credential storage adapter (keyring crate) |
| `git-vacuum-service` | Orchestration layer — sync engine, discovery, auth, merge |
| `git-vacuum-app` | Redux-style app state and reducers |
| `git-vacuum-tui` | Ratatui terminal UI rendering |
| `git-vacuum` | Binary crate — composition root, CLI, main loop |

Service, app, and TUI crates depend only on `core` traits — never on concrete adapters. The binary wires everything together.

**Unidirectional data flow:** Input (TUI) → Action → Reducer (app) → Effect → Background tasks → AppEvent → Reducer → Render.

Design documents: `docs/design/`.

## Security

- Token stored in **OS keyring only** (Windows Credential Manager, macOS Keychain, Secret Service on Linux). Never in SQLite, logs, error messages, or plaintext files.
- No backend. GitHub API is the only outbound network dependency.
- All data stays on the user's machine.

## License

MIT — see [LICENSE](LICENSE).
