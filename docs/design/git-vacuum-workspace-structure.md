# Git-Vacuum — Final Rust Workspace Structure

**Pattern:** Hexagonal architecture (ports & adapters) expressed as a Cargo workspace  
**Core principle:** Infrastructure crates never import service or app crates. Service crate depends only on traits from core, never on concrete implementations. The binary crate is the sole wire-up point.

---

## 1. Workspace Layout

```
git-vacuum/                         ← repo root
│
├── Cargo.toml                      ← workspace manifest
├── Cargo.lock
├── README.md
├── LICENSE
├── .gitignore
├── rust-toolchain.toml             ← pins Rust version
│
├── crates/
│   │
│   ├── git-vacuum-core/            ← shared types, traits, events, errors
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-db/              ← SQLite infrastructure
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-github/          ← GitHub API infrastructure (Octocrab)
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-git/             ← Git operations infrastructure (git2-rs)
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-keyring/         ← OS credential storage infrastructure
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-service/         ← Service orchestration (sync engine, discovery, auth)
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-app/             ← Application state, reducer, tab routing
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── git-vacuum-tui/             ← Ratatui terminal UI (screens, components)
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   └── git-vacuum/                 ← Binary crate (main loop, CLI, wiring)
│       ├── Cargo.toml
│       └── src/
│
├── docs/
│   └── architecture/               ← rendered design docs (optional)
│
└── .commandcode/
    └── plans/                      ← all design documents
```

---

## 2. Crate Inventory

### 2.1 `git-vacuum-core` — Shared Kernel

**Purpose:** The single source of truth for all types, traits, events, and errors shared across the workspace. Zero external dependencies beyond `serde`, `thiserror`, and `tokio` (for channel types and async trait support).

**Cargo.toml dependencies:**
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tokio = { version = "1", features = ["sync", "time"] }
chrono = { version = "0.4", features = ["serde"] }
```

**Module tree:**
```
src/
├── lib.rs                     # Re-exports everything
│
├── types/
│   ├── mod.rs                 # Re-exports
│   ├── repo.rs                # RemoteRepo, RepoEntry, RepoVisibility
│   ├── user.rs                # UserInfo, AuthMethod
│   ├── org.rs                 # OrgInfo
│   ├── sync.rs                # SyncOptions, CloneProtocol, SyncSummary, PartialSyncSummary
│   ├── job.rs                 # JobId, JobSpec, PlannedOperation, SkipReason, Priority
│   ├── progress.rs            # ProgressSample, OverallProgress, ActiveJobProgress, JobPhase
│   ├── activity.rs            # SyncRunRow, SyncEntryRow, ExportFormat
│   └── settings.rs            # SettingsCategory, SettingsField
│
├── traits/
│   ├── mod.rs                 # Re-exports all traits
│   ├── database.rs            # Database trait + data row types (RepoRow, etc.)
│   ├── github_api.rs          # GithubApi trait
│   ├── git_ops.rs             # GitOps trait
│   └── keyring_store.rs       # KeyringStore trait
│
├── event.rs                   # InputEvent, Action, AppEvent, Effect, EventBus
├── error.rs                   # SyncError, DiscoveryError, AuthError, ErrorKind
└── util.rs                    # Shared helpers (exponential_backoff, human_bytes, etc.)
```

**Ownership rule:** This crate owns the canonical definition of every type used across crate boundaries. No other crate may define its own `RepoInfo` or `AppEvent` — they import from `git-vacuum-core`. This prevents the "multiple incompatible type definitions" problem common in large workspaces.

### 2.2 `git-vacuum-db` — SQLite Adapter

**Purpose:** Concrete implementation of the `Database` trait. Manages SQLite connection, migrations, and all queries. This crate is the only crate that imports `rusqlite`.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
rusqlite = { version = "0.31", features = ["bundled", "backup"] }
tokio = { version = "1", features = ["sync"] }
log = "0.4"
```

**Module tree:**
```
src/
├── lib.rs                     # SqliteDatabase struct, Database trait impl
├── connection.rs              # Connection pool, WAL mode, PRAGMA setup
├── migrations.rs              # include_dir! embedded SQL files, migration runner
├── queries/
│   ├── mod.rs                 # Re-exports
│   ├── repos.rs               # upsert_repos, get_all_repos, update_local_status
│   ├── sync_runs.rs           # insert_sync_run, get_sync_runs, update_sync_run
│   ├── sync_entries.rs        # insert_sync_entries, get_sync_entries
│   ├── sync_log.rs            # insert_log_entries, get_log_entries, cleanup
│   ├── accounts.rs            # upsert_account, get_account
│   ├── orgs.rs                # upsert_orgs, get_orgs
│   ├── settings.rs            # get_setting, set_setting, get_all_settings
│   └── stats.rs               # get_dashboard_stats, get_attention_list
└── migrations/
    └── 001_initial_schema.sql # All CREATE TABLE + seed data
```

### 2.3 `git-vacuum-github` — GitHub API Adapter

**Purpose:** Concrete implementation of the `GithubApi` trait. Wraps Octocrab. Handles authentication, pagination, rate limiting, and API-to-domain type mapping.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
octocrab = { version = "0.42", features = ["retry", "stream"] }
tokio = { version = "1", features = ["sync", "time", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
log = "0.4"
thiserror = "2"
```

**Module tree:**
```
src/
├── lib.rs                     # OctocrabGithubApi struct, GithubApi trait impl
├── client.rs                  # OctocrabBuilder configuration, token attachment
├── auth.rs                    # validate_token, device_flow_init, device_flow_poll
├── user.rs                    # get_authenticated_user
├── repos.rs                   # list_my_repos, list_org_repos, list_starred_repos
├── orgs.rs                    # list_my_orgs
├── pagination.rs              # PagedStream<T> — rate-limit-aware page iterator
├── rate_limit.rs              # RateLimiter — pre-flight checks, retry with backoff
├── mapping.rs                 # octocrab::Repository → RemoteRepo, etc.
└── error.rs                   # GithubError, error classification
```

### 2.4 `git-vacuum-git` — Git Operations Adapter

**Purpose:** Concrete implementation of the `GitOps` trait. Wraps git2-rs. Performs clone, fetch, status, and mirror operations. All git2-rs interaction is confined to this crate.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
git2 = { version = "0.19" }
tokio = { version = "1", features = ["sync", "process"] }
log = "0.4"
thiserror = "2"
```

**Module tree:**
```
src/
├── lib.rs                     # Git2GitOps struct, GitOps trait impl
├── clone.rs                   # clone_repo(url, path, progress_cb, cancel_rx) → CloneResult
├── fetch.rs                   # fetch(path, cancel_rx) → FetchResult
├── sync.rs                    # fetch_and_fast_forward(path) → SyncResult
├── status.rs                  # local_status(path) → LocalRepoStatus
├── mirror.rs                  # mirror_clone(url, path) → CloneResult (stub for MVP)
├── util.rs                    # is_git_repo, resolve_url, find_git_binary
└── error.rs                   # GitError — maps git2 errors to domain errors
```

### 2.5 `git-vacuum-keyring` — Credential Storage Adapter

**Purpose:** Concrete implementation of the `KeyringStore` trait. Wraps the `keyring` crate for platform-native secure storage. Falls back to encrypted file on platforms without keyring support.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
keyring = { version = "3" }
log = "0.4"
thiserror = "2"
```

**Module tree:**
```
src/
├── lib.rs                     # PlatformKeyring struct, KeyringStore trait impl
└── platform.rs                # OS-specific keyring backend selection
```

### 2.6 `git-vacuum-service` — Service Orchestration

**Purpose:** The orchestration layer. Contains the sync engine, discovery logic, auth service, activity recording, and stats computation. Depends ONLY on `git-vacuum-core` traits — never on concrete infrastructure crates. All infrastructure is injected via trait objects at construction time.

**This is the most critical architectural boundary in the entire codebase.** If this crate compiles without any infrastructure crate in its `Cargo.toml`, the hexagonal architecture is intact.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
tokio = { version = "1", features = ["sync", "time", "macros", "rt"] }
futures = "0.3"
log = "0.4"
thiserror = "2"
serde_json = "1"
# NO dependency on git-vacuum-db, git-vacuum-github, git-vacuum-git, git-vacuum-keyring
```

**Module tree:**
```
src/
├── lib.rs                     # Services struct (holds Arc<dyn Trait> for each adapter)
│                              # run_sync(), discover(), authenticate() entry points
│
├── sync_engine/
│   ├── mod.rs                 # Re-exports, SyncEngine struct
│   ├── coordinator.rs         # run_sync() — top-level pipeline orchestration
│   ├── plan.rs                # resolve_plan() — determines Clone/Sync/Skip per repo
│   ├── queue.rs               # JobQueue — priority dispatch, active tracking, retry queue
│   ├── dispatcher.rs          # Dispatcher loop: dequeue → acquire semaphore → spawn worker
│   ├── worker.rs              # execute_job() — single clone/sync/mirror operation
│   ├── retry.rs               # should_retry() → RetryDecision, backoff calculation
│   ├── collector.rs           # Result collector: aggregates outcomes, drives retry queue
│   ├── progress.rs            # ProgressTracker — aggregation, throughput window, ETA
│   └── events.rs              # Sync engine → AppEvent conversion functions
│
├── discovery.rs               # discover_repos() — orchestrate API + DB + filesystem merge
├── auth_service.rs            # authenticate_pat(), authenticate_oauth_device_flow()
├── activity.rs                # record_sync_run(), record_sync_entries(), query_history()
├── stats.rs                   # compute_dashboard_stats(), compute_attention_list()
└── merge.rs                   # Repository merge: remote + cached + filesystem → unified view
```

### 2.7 `git-vacuum-app` — Application State

**Purpose:** The Redux-style application layer. Owns `App` state, the reducer functions, tab navigation, and modal stack. Depends on service traits (for effect types) but not on service implementations.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
tokio = { version = "1", features = ["sync"] }
log = "0.4"
# NO dependency on git-vacuum-service, git-vacuum-db, etc.
```

**Module tree:**
```
src/
├── lib.rs                     # App struct, App::new(), App::should_quit()
│
├── state.rs                   # AppState enum (Auth | Running | FatalError)
│                              # RunningAppState: active_tab, tab_states, modal_stack
│
├── tabs/
│   ├── mod.rs                 # Tab enum, TabStates struct
│   ├── dashboard.rs           # DashboardTabState
│   ├── explorer.rs            # ExplorerTabState
│   ├── sync_center.rs         # SyncCenterTabState, SyncPhase enum
│   ├── activity_log.rs        # ActivityLogTabState
│   └── settings.rs            # SettingsTabState
│
├── modals.rs                  # Modal enum, ModalStack (Vec<Modal>), focus trapping
├── command_palette.rs         # CommandPaletteState, command registry, fuzzy matching
│
├── reduce.rs                  # reduce(&mut App, Action) → Vec<Effect>
│                              # reduce_event(&mut App, AppEvent) → Vec<Effect>
│                              # The ONLY functions that mutate App
│
└── effects.rs                 # Effect enum → side effect descriptors (no async code)
```

### 2.8 `git-vacuum-tui` — Terminal UI

**Purpose:** Ratatui-based rendering layer. Reads `&App` immutably. Maps keyboard input to `Action` values. Never mutates app state directly. Every screen and component lives here.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
git-vacuum-app = { path = "../git-vacuum-app" }
ratatui = { version = "0.29", features = ["all-widgets"] }
crossterm = { version = "0.28" }
unicode-width = "0.2"
log = "0.4"
```

**Module tree:**
```
src/
├── lib.rs                     # render(frame, &app) → main entry point for rendering
│
├── terminal.rs                # Crossterm setup/teardown, raw mode, alt screen, event poll
├── input.rs                   # map_key_to_action(key, &app) → Action (context-aware)
├── theme.rs                   # Color constants, style builders, semantic color map
│
├── layout/
│   ├── mod.rs                 # Re-exports
│   ├── shell.rs               # Outer shell: title bar, tab bar, key bar, breadcrumb
│   └── constraints.rs         # Layout helpers for responsive panels
│
├── components/
│   ├── mod.rs                 # Re-exports all components
│   ├── table.rs               # Selectable, scrollable, multi-column data table
│   ├── tabs.rs                # Tab bar with highlight, count badges
│   ├── key_bar.rs             # Dynamic key hint bar (changes per screen + modal)
│   ├── title_bar.rs           # Two-row title bar with stats, user info
│   ├── breadcrumb.rs          # Navigation path trail
│   ├── progress.rs            # Gauge widget, throughput display, ETA string
│   ├── spinner.rs             # Animated spinner (frame from tick_count % N)
│   ├── modal.rs               # Modal overlay: backdrop dim, border, focus trapping
│   ├── command_palette.rs     # Fuzzy-search command palette widget
│   ├── help_overlay.rs        # Keyboard reference overlay (? key)
│   ├── confirm_dialog.rs      # Yes/No confirmation popup
│   ├── input_field.rs         # Text input with cursor, masking, placeholder
│   ├── dropdown.rs            # Dropdown selector widget
│   ├── radio_group.rs         # Radio button group widget
│   ├── chart.rs               # Simple bar chart (for size distribution)
│   └── scrollable_text.rs     # Long text with scroll support
│
└── screens/
    ├── mod.rs                 # Screen trait, screen registry, render dispatch
    ├── auth.rs                # Auth gate screen (PAT input, OAuth flow, status)
    ├── dashboard.rs           # Dashboard: sync health, attention list, size chart
    ├── explorer.rs            # Explorer: source selector, filter, repo table, detail panel
    ├── sync_center.rs         # Sync: pre-sync, active progress, post-sync results
    ├── activity_log.rs        # Activity: run history table, run detail panel
    └── settings.rs            # Settings: sidebar nav + form fields per category
```

### 2.9 `git-vacuum` — Binary Crate

**Purpose:** The executable. Parses CLI args, initializes all subsystems, creates the event bus, wires concrete implementations to traits, runs the main loop. The ONLY crate that depends on everything.

**Cargo.toml dependencies:**
```toml
[dependencies]
git-vacuum-core = { path = "../git-vacuum-core" }
git-vacuum-db = { path = "../git-vacuum-db" }
git-vacuum-github = { path = "../git-vacuum-github" }
git-vacuum-git = { path = "../git-vacuum-git" }
git-vacuum-keyring = { path = "../git-vacuum-keyring" }
git-vacuum-service = { path = "../git-vacuum-service" }
git-vacuum-app = { path = "../git-vacuum-app" }
git-vacuum-tui = { path = "../git-vacuum-tui" }
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
log = "0.4"
env_logger = "0.11"
dirs = "6"
anyhow = "1"
```

**Module tree:**
```
src/
├── main.rs                    # Entry point, tokio runtime, main loop
├── config.rs                  # Clap CLI args + env var fallback parsing
├── wiring.rs                  # Dependency injection: create Services, wire adapters
└── signal.rs                  # OS signal handling (SIGINT → graceful shutdown)
```

---

## 3. Workspace Manifest

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "3"
members = [
    "crates/git-vacuum-core",
    "crates/git-vacuum-db",
    "crates/git-vacuum-github",
    "crates/git-vacuum-git",
    "crates/git-vacuum-keyring",
    "crates/git-vacuum-service",
    "crates/git-vacuum-app",
    "crates/git-vacuum-tui",
    "crates/git-vacuum",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/PrakashSewani/git-vacuum"
rust-version = "1.85"

[workspace.dependencies]
# Shared version pins — all crates reference these
tokio = { version = "1", features = ["sync"] }
thiserror = "2"
serde = { version = "1", features = ["derive"] }
log = "0.4"
```

---

## 4. Dependency Graph

### 4.1 Compile-Time Graph

```
                    ┌─────────────────┐
                    │  git-vacuum     │  ◀── Binary (wires everything)
                    └───────┬─────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ git-vacuum-tui│   │ git-vacuum-app│   │git-vacuum-db  │
└───────┬───────┘   └───────┬───────┘   │git-vacuum-gh  │
        │                   │           │git-vacuum-git │
        │                   │           │git-vacuum-key │
        │                   │           └───────┬───────┘
        │                   │                   │
        │                   ▼                   │
        │           ┌───────────────┐           │
        │           │git-vacuum-svc │ (traits   │
        │           │               │  only)    │
        │           └───────┬───────┘           │
        │                   │                   │
        └───────────────────┼───────────────────┘
                            │
                            ▼
                    ┌───────────────┐
                    │ git-vacuum-   │  ◀── Shared kernel
                    │    core       │      (types, traits, events)
                    └───────────────┘
```

### 4.2 Dependency Table

| Crate | Depends On | Depended By |
|-------|-----------|------------|
| `git-vacuum-core` | Nothing (leaf) | Everything |
| `git-vacuum-db` | core | binary |
| `git-vacuum-github` | core | binary |
| `git-vacuum-git` | core | binary |
| `git-vacuum-keyring` | core | binary |
| `git-vacuum-service` | **core only** (traits) | app, binary |
| `git-vacuum-app` | core, service | tui, binary |
| `git-vacuum-tui` | core, app | binary |
| `git-vacuum` (binary) | all of the above | nothing |

### 4.3 Forbidden Dependencies

These imports must never appear in a Cargo.toml. Enforced by code review and (optionally) a CI lint script:

| From | Must NOT import | Because |
|------|----------------|---------|
| `git-vacuum-service` | `git-vacuum-db` | Service layer must depend on traits, not implementations |
| `git-vacuum-service` | `git-vacuum-github` | Same — infrastructure must be injectable |
| `git-vacuum-service` | `git-vacuum-git` | Same |
| `git-vacuum-service` | `git-vacuum-keyring` | Same |
| `git-vacuum-app` | `git-vacuum-db` | App layer must not reach into infrastructure |
| `git-vacuum-app` | `git-vacuum-github` | Same |
| `git-vacuum-app` | `git-vacuum-git` | Same |
| `git-vacuum-app` | `git-vacuum-keyring` | Same |
| `git-vacuum-tui` | `git-vacuum-db` | TUI reads state from App, never from DB directly |
| `git-vacuum-tui` | `git-vacuum-service` | TUI sends Actions to reducer, never calls service directly |
| `git-vacuum-core` | anything | Core is the leaf — zero workspace dependencies |

**Enforcement script (conceptual):**
```
# CI step: verify no forbidden workspace dependencies
for crate in git-vacuum-service git-vacuum-app git-vacuum-tui; do
    for forbidden in git-vacuum-db git-vacuum-github git-vacuum-git git-vacuum-keyring; do
        if cargo tree -p $crate --depth 1 | grep -q $forbidden; then
            echo "ERROR: $crate must not depend on $forbidden"
            exit 1
        fi
    done
done
```

---

## 5. Ownership Boundaries

### 5.1 What Each Crate Owns

| Crate | Owns | Does NOT own |
|-------|------|-------------|
| `core` | Type definitions, trait signatures, event enum, error enum | No behavior, no I/O, no async code (except channel types) |
| `db` | SQLite connection, schema, migrations, all SQL queries | Business logic. Never decides WHAT to store, only HOW |
| `github` | Octocrab client, API endpoint calls, rate limit state, pagination | Discovery strategy. Returns raw data, never merges |
| `git` | git2-rs calls, clone/fetch/status operations, progress callbacks | Sync orchestration. Never decides which repos to clone |
| `keyring` | Platform credential storage, token set/get/delete | Token validation. Only stores what it's told |
| `service` | Sync pipeline, discovery merge, auth orchestration, stats computation | UI, database schema, git specifics, HTTP specifics |
| `app` | App state, reducer, tab routing, modal stack, command palette registry | Rendering, infrastructure, effects execution |
| `tui` | Ratatui widgets, screen composition, key mapping, terminal setup | Application state mutation, business logic, I/O |
| `binary` | CLI args, dependency injection, main loop, effect execution, signal handling | Domain logic — delegates everything |

### 5.2 The `Services` struct (in `binary/wiring.rs`)

This is the single point where concrete implementations meet traits. Only the binary crate constructs this:

```
// In binary crate (wiring.rs)
pub struct Services {
    pub github: Arc<dyn GithubApi>,
    pub git: Arc<dyn GitOps>,
    pub db: Arc<dyn Database>,
    pub keyring: Arc<dyn KeyringStore>,
}

pub fn create_services(config: &AppConfig) -> Result<Services> {
    let keyring = Arc::new(PlatformKeyring::new("git-vacuum")?);
    let db = Arc::new(SqliteDatabase::open(&config.db_path)?);
    db.run_migrations()?;
    let github = Arc::new(OctocrabGithubApi::new(
        config.github_base_url.clone(),
        config.user_agent.clone(),
    ));
    let git = Arc::new(Git2GitOps::new(config.git_binary.clone()));
    Ok(Services { github, git, db, keyring })
}
```

`Services` is passed to the app layer's `App::new()`, which stores it. Effect executors in the binary crate clone `Arc<Services>` to pass individual `Arc<dyn Trait>` handles to background tasks.

### 5.3 Cross-Crate Visibility

All types shared between crates live in `git-vacuum-core` and are `pub`. Types internal to a crate are `pub(crate)` or private. No crate exposes internal implementation details.

**Example:**
- `RemoteRepo` is in `core::types::repo` → `pub`
- `OctocrabGithubApi` is in `github::lib` → `pub` (binary crate needs to construct it)
- `SqliteDatabase::connection_pool` is private — only `SqliteDatabase` methods access it
- `SyncEngine::semaphore` is `pub(crate)` — only within `git-vacuum-service`

---

## 6. Feature Flags

Feature flags control optional functionality and allow conditional compilation. For MVP, none are needed. For v1.0+:

```toml
# git-vacuum-github/Cargo.toml (future)
[features]
default = []
enterprise = []  # GitHub Enterprise server support (different auth, different base URLs)

# git-vacuum-git/Cargo.toml (future)
[features]
default = []
lfs = ["git2/vendored-libgit2"]  # Git LFS support

# git-vacuum-tui/Cargo.toml
[features]
default = ["nerd-fonts"]
nerd-fonts = []   # Use Nerd Font icons (utf-8 private use area characters)
ascii-icons = []  # ASCII fallback: [OK] instead of ✓
```

---

## 7. Build & Test Strategy

### 7.1 Building

```bash
# Build everything
cargo build --workspace

# Build only the binary
cargo build -p git-vacuum

# Build with release optimizations
cargo build --workspace --release
```

### 7.2 Testing

```bash
# Run all tests (each crate has its own #[cfg(test)] modules)
cargo test --workspace

# Run only core tests (fastest, no external dependencies)
cargo test -p git-vacuum-core

# Run only db tests (requires SQLite)
cargo test -p git-vacuum-db

# Run only service tests (mocked infrastructure)
cargo test -p git-vacuum-service

# Run with all features (future)
cargo test --workspace --all-features
```

### 7.3 Test Isolation

Each crate tests its own behavior:
- **core:** Pure function tests. `reduce()` state transitions, type serialization, event construction.
- **db:** Integration tests with in-memory SQLite (`:memory:` connection).
- **github:** Mock HTTP server tests (using `wiremock` or similar). OR skip integration tests in CI by default (they require a real GitHub token).
- **git:** Integration tests with temp directories. Create a small git repo, clone it, verify.
- **keyring:** Platform-dependent tests, skipped in CI by default.
- **service:** Mock implementations of all four traits. Inject mocks via `Arc<dyn Trait>`. Test sync engine with a mock `GitOps` that returns predetermined results. Test discovery merge logic with mock `GithubApi` + mock `Database`.
- **app:** Pure function tests of the reducer. Construct a state, apply an action, assert new state + effects.
- **tui:** Buffer tests. Render a screen to a `ratatui::buffer::Buffer`, assert specific cells contain expected text. Snapshot tests for layout regression.

### 7.4 CI Pipeline

```yaml
# Conceptual CI steps
jobs:
  - cargo fmt --check --all
  - cargo clippy --workspace -- -D warnings
  - cargo test --workspace --exclude git-vacuum-github --exclude git-vacuum-keyring
  - cargo test -p git-vacuum-github -- --ignored  # if we mark real-API tests as #[ignore]
  - cargo build --workspace --release
  - cargo tree -p git-vacuum-service | grep -E 'git-vacuum-(db|github|git|keyring)' && exit 1 || true
```

---

## 8. Workspace Size Rationale

Nine crates in a workspace might seem like over-engineering for an MVP. Here's why it's the right call now:

**Compile-time wins:**
- `git-vacuum-core` is a thin leaf crate. Changing a UI string doesn't recompile the database layer.
- `git-vacuum-tui` is the most frequently changed crate during development. It depends on `core` and `app` — not on `db`, `github`, or `git`. Changing a Ratatui layout doesn't recompile `rusqlite` or `git2`.
- The binary crate is tiny (~200 lines: main loop + wiring). Changes to it don't force recompilation of anything else.

**Test wins:**
- Service tests run without linking `rusqlite` or `octocrab` or `git2`. Dependencies are mocked at the trait boundary. This makes service tests fast (compile + run in seconds).
- Core tests are practically instant.

**Architectural enforcement:**
- The forbidden-dependency check is a CI script. It prevents architectural erosion — no developer can accidentally import `rusqlite` into the service layer because it's not in the `Cargo.toml`.
- New team members can understand the architecture by reading the workspace `Cargo.toml` dependencies — the graph IS the documentation.

**Cost:**
- 9 `Cargo.toml` files to maintain (one-time setup, minimal ongoing changes).
- Slightly more verbose imports (`use git_vacuum_core::types::repo::RemoteRepo` vs `use crate::types::repo::RemoteRepo`). Mitigated by re-exports in each crate's `lib.rs`.

---

## 9. Crate Re-Export Convention

Every crate's `lib.rs` re-exports the public API in a flat namespace to make imports ergonomic:

```rust
// git-vacuum-core/src/lib.rs
pub mod types;
pub mod traits;
pub mod event;
pub mod error;
pub mod util;

// Re-export commonly used items
pub use types::repo::RemoteRepo;
pub use types::user::UserInfo;
pub use types::sync::{SyncOptions, SyncSummary, CloneProtocol};
pub use traits::database::Database;
pub use traits::github_api::GithubApi;
pub use traits::git_ops::GitOps;
pub use traits::keyring_store::KeyringStore;
pub use event::{Action, AppEvent, Effect, EventBus, InputEvent};
pub use error::SyncError;
```

This allows consumers to write:

```rust
use git_vacuum_core::{RemoteRepo, GithubApi, Action};
```

Instead of:

```rust
use git_vacuum_core::types::repo::RemoteRepo;
use git_vacuum_core::traits::github_api::GithubApi;
use git_vacuum_core::event::Action;
```

---

## 10. Summary

| Crate | Lines (est.) | Dependencies | Test Strategy |
|-------|-------------|-------------|--------------|
| `git-vacuum-core` | ~800 | tokio, serde, thiserror, chrono | Pure unit tests |
| `git-vacuum-db` | ~600 | core, rusqlite | In-memory SQLite |
| `git-vacuum-github` | ~1,200 | core, octocrab, serde_json | Mock HTTP or ignored |
| `git-vacuum-git` | ~500 | core, git2 | Temp directories |
| `git-vacuum-keyring` | ~100 | core, keyring | Platform-dependent, CI-skipped |
| `git-vacuum-service` | ~2,000 | **core only**, tokio, futures | Mocked infrastructure |
| `git-vacuum-app` | ~800 | core, service (traits) | Pure state transition tests |
| `git-vacuum-tui` | ~2,500 | core, app, ratatui, crossterm | Buffer/snapshot tests |
| `git-vacuum` (binary) | ~300 | all of the above | Integration smoke test |
| **Total** | **~8,800** | | |

The workspace enforces the architectural rules at the compiler level, keeps build times low during development, and makes every subsystem independently testable.
