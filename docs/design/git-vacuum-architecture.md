# Git-Vacuum — Application Architecture

**Stack:** Rust + Ratatui + Tokio + Octocrab + SQLite + git2-rs  
**Pattern:** Unidirectional data flow (Elm/Redux-inspired) with effect-based side effects  
**Constraint:** All UI interaction is keyboard-first, all blocking I/O is async via Tokio

---

## 1. Layered Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    COMPOSITION ROOT                           │
│  main.rs — runtime init, wire-up, main loop                 │
├──────────────────────────────────────────────────────────────┤
│                    PRESENTATION LAYER                         │
│  ui/ — Ratatui rendering, input handling, screen composition│
│  Depends on: app (for state reading only)                    │
├──────────────────────────────────────────────────────────────┤
│                    APPLICATION LAYER                          │
│  app.rs — App state, reducer, tab routing                    │
│  event.rs — Event/Action enums, EventBus, Effect enum       │
│  config.rs — CLI arg parsing (clap)                          │
│  Depends on: service (for Effect execution)                  │
├──────────────────────────────────────────────────────────────┤
│                    SERVICE LAYER                              │
│  service/ — Orchestration: sync engine, discovery, auth     │
│  Depends on: db, github, git, keyring (all via traits)       │
├──────────────────────────────────────────────────────────────┤
│                    INFRASTRUCTURE LAYER                       │
│  db/ — SQLite (rusqlite), migrations, queries                │
│  github/ — Octocrab client, API wrappers                     │
│  git/ — git2-rs clone/fetch/status operations               │
│  keyring/ — OS credential storage                            │
│  Depends on: nothing above this layer                        │
└──────────────────────────────────────────────────────────────┘
```

**The cardinal rule:** Dependencies flow strictly downward. Infrastructure never imports from Service. Service never imports from Application. Only the Composition Root references every layer for wiring.

**Rationale for each dependency direction:**
- `ui` reads `App` state immutably (shared reference) — it never mutates state directly.
- `ui` maps key events to `Action` values and feeds them to the reducer.
- `app::reduce()` is the sole mutator of `App` state. It returns `Vec<Effect>` for side effects.
- `main` loop spawns `Effect` values as Tokio tasks using `service` layer functions.
- `service` functions accept infrastructure traits as parameters (injected at startup).

---

## 2. Module Structure

```
src/
├── main.rs                       # Entry: tokio runtime, terminal setup, main loop
├── app.rs                        # App struct, AppState, reduce(), tab navigation
├── event.rs                      # InputEvent, Action, AppEvent, Effect, EventBus
├── config.rs                     # Clap CLI definition + env var fallbacks
│
├── db/
│   ├── mod.rs                    # Re-exports, Database trait + SqliteDatabase impl
│   ├── connection.rs             # SQLite connection pool (r2d2-sqlite or deadpool-sqlite)
│   ├── migrations.rs             # Schema versioning, up/down migrations
│   ├── models.rs                 # Row types: RepoRow, SyncRunRow, SyncEntryRow, SettingsRow
│   └── queries.rs                # Repository pattern: insert/update/query methods
│
├── github/
│   ├── mod.rs                    # Re-exports, GithubApi trait + OctocrabGithubApi impl
│   ├── client.rs                 # Octocrab instance builder (auth, user-agent, base URL)
│   ├── repos.rs                  # list_owned(), list_org(), list_starred(), list_all()
│   ├── user.rs                   # get_authenticated_user()
│   └── auth.rs                   # validate_token(), device_flow_init(), device_flow_poll()
│
├── git/
│   ├── mod.rs                    # Re-exports, GitOps trait + Git2GitOps impl
│   ├── clone.rs                  # clone_repo(url, path) -> progress stream
│   ├── sync.rs                   # fetch_and_fast_forward(path) -> new_commit_count
│   ├── status.rs                 # local_status(path) -> (behind_count, ahead_count, clean)
│   └── mirror.rs                 # mirror_clone(url, path) — stub in MVP, full in v1.0
│
├── keyring/
│   ├── mod.rs                    # Re-exports, KeyringStore trait + PlatformKeyring impl
│   └── platform.rs               # keyring crate integration, fallback to encrypted file
│
├── service/
│   ├── mod.rs                    # Services struct — groups all infra dependencies
│   ├── sync_engine.rs            # CloneAll, SyncAll — worker pool, progress streaming
│   ├── discovery.rs              # discover_repos() — merges GitHub + DB + filesystem state
│   ├── auth_service.rs           # authenticate() — full auth flow orchestration
│   ├── activity.rs               # record_run(), record_entry(), query_runs()
│   └── stats.rs                  # compute_dashboard_stats() — aggregates DB + local git
│
└── ui/
    ├── mod.rs                    # Re-exports, render() entry point
    ├── terminal.rs               # Crossterm setup: raw mode, alternate screen, event poll
    ├── theme.rs                  # Color constants, style builders, semantic color map
    ├── input.rs                  # KeyEvent → Action mapping (context-aware via AppState)
    │
    ├── components/
    │   ├── mod.rs
    │   ├── table.rs              # Selectable, scrollable, multi-column data table
    │   ├── tabs.rs               # Tab bar with highlight, count badges
    │   ├── key_bar.rs            # Dynamic key hint bar per screen
    │   ├── title_bar.rs          # Two-row title bar with stats
    │   ├── breadcrumb.rs         # Navigation path trail
    │   ├── progress.rs           # Gauge, throughput display, ETA calculator
    │   ├── spinner.rs            # Animated spinner (frames from tick_count)
    │   ├── modal.rs              # Modal overlay: backdrop dim, border, focus trapping
    │   ├── command_palette.rs    # Fuzzy-search command palette
    │   ├── help.rs               # Keybinding reference overlay
    │   ├── confirm.rs            # Yes/No confirmation dialog
    │   └── input_field.rs        # Text input with cursor, masking (for tokens)
    │
    └── screens/
        ├── mod.rs                # Screen trait, screen registry
        ├── auth.rs               # Auth gate screen (first-run or expired token)
        ├── dashboard.rs          # Dashboard: health gauge, attention list, size chart
        ├── explorer.rs           # Explorer: source selector, filter, repo table, detail panel
        ├── sync_center.rs        # Sync: pre-sync summary, live progress, post-sync results
        ├── activity_log.rs       # Activity: sync history table, run detail panel
        └── settings.rs           # Settings: sidebar nav, form fields per category
```

---

## 3. Event Bus

### Architecture

The event system follows a **unidirectional loop**:

```
┌──────────┐     Action      ┌──────────┐     Effect      ┌──────────────┐
│  INPUT   │ ────────────── ▶ │ REDUCER  │ ────────────── ▶│  BACKGROUND  │
│ HANDLER  │                 │ app.rs   │                 │   TASKS      │
│ui/input  │                 │          │                 │  (tokio)     │
└──────────┘                 └──────────┘                 └──────┬───────┘
                                  ▲                              │
                                  │         AppEvent              │
                                  └──────────────────────────────┘
                                         (via EventBus)
```

**Rationale for this design:**
- The reducer is the sole mutator of state — no race conditions, no locks on app state.
- Side effects (network, filesystem) happen in background tasks, never in the reducer.
- Background tasks communicate results back through typed events — no shared mutable state between tasks.
- The UI is a pure function of state at render time — easy to reason about and test.

### Core Types

#### InputEvent
Raw terminal events before mapping to actions:

```
InputEvent::Key(KeyEvent)
InputEvent::Resize(u16, u16)
InputEvent::Tick              // emitted every ~16ms for animations
```

#### Action
User intent. Mapped from key events in `ui/input.rs`. The mapping is context-aware (varies per screen and per modal state):

```
enum Action {
    // ── Tab Navigation ──
    SwitchTab(Tab),
    NextTab,
    PrevTab,

    // ── Global ──
    Quit,
    OpenHelp,
    OpenCommandPalette,
    DismissModal,              // Esc — pops top of modal stack
    ConfirmModal,              // Enter on focused modal button
    NoOp,                      // unmapped key

    // ── Explorer ──
    ExplorerToggle(usize),     // Space on row
    ExplorerSelectAll,         // Ctrl+A
    ExplorerDeselectAll,       // Ctrl+D
    ExplorerStartMarkMode,     // v
    ExplorerMarkTo(usize),     // extend mark range (Shift+↓/↑)
    ExplorerEndMarkMode,       // Esc or second v
    ExplorerSetFilter(String),
    ExplorerClearFilter,
    ExplorerSortColumn(u8),    // 1-6
    ExplorerStartSync,         // Enter on multi-select
    ExplorerInspect(usize),    // Enter on single row → opens RepoDetail modal
    ExplorerOpenBrowser(usize),

    // ── Sync Center ──
    SyncStart,                 // Enter from pre-sync view
    SyncPause,                 // p during active sync
    SyncResume,                // r when paused
    SyncCancel,                // c with confirmation
    SyncShowErrorsOnly,        // e
    SyncShowAll,               // a
    SyncToggleFollow,          // f
    SyncScrollUp,              // ↑
    SyncScrollDown,            // ↓
    SyncViewFailedDetails,     // Enter on post-sync "View Failed" button

    // ── Dashboard ──
    DashboardRefreshStats,     // r
    DashboardStartSync,        // s
    DashboardInspect(usize),   // Enter on attention list item

    // ── Activity Log ──
    ActivityViewRun(usize),    // Enter on run row
    ActivityRetryRun(usize),   // r
    ActivityExportRun(usize),  // e
    ActivitySetFilter(String),

    // ── Settings ──
    SettingsNavigate(usize),   // ↑↓ in field list
    SettingsEdit(usize),       // Enter on field → enter edit mode
    SettingsToggle(usize),     // Space on boolean field
    SettingsSelectDropdown(usize), // Enter on dropdown
    SettingsDropdownPick(usize),   // pick item in open dropdown
    SettingsSave,              // Ctrl+S
    SettingsDiscard,           // Esc from edit mode
    SettingsSwitchCategory(usize), // Tab in sidebar

    // ── Auth ──
    AuthSubmitToken(String),   // Enter on token input
    AuthStartOAuth,            // Select OAuth method
    AuthStartPAT,              // Select PAT method
    AuthCancelOAuth,           // Esc during OAuth poll
    AuthSkipForPublic,         // Skip auth button

    // ── Command Palette ──
    CommandPaletteFilter(String),
    CommandPaletteExecute(String),
    CommandPaletteDismiss,
}
```

#### AppEvent
Events emitted by background tasks and consumed by the reducer:

```
enum AppEvent {
    // ── Auth ──
    AuthSucceeded {
        username: String,
        scopes: Vec<String>,
        token_expires: Option<DateTime>,
    },
    AuthFailed {
        reason: String,           // "invalid_token", "network_error", "insufficient_scopes"
        detail: String,
    },
    OAuthCodeReceived {
        user_code: String,
        verification_uri: String,
        expires_in: Duration,
    },
    OAuthTokenReceived {
        token: String,
    },
    OAuthTimeout,

    // ── Discovery ──
    ReposDiscovered {
        repos: Vec<RemoteRepo>,   // raw GitHub API data
        source: RepoSource,       // "my_repos", "org:acme", "starred", "all"
    },
    DiscoveryFailed {
        error: String,
    },

    // ── Sync Progress (streamed per-repo) ──
    SyncCloneStarted {
        repo_full_name: String,
    },
    SyncCloneProgress {
        repo_full_name: String,
        bytes_received: u64,
        total_bytes: u64,
    },
    SyncCloneCompleted {
        repo_full_name: String,
        size_bytes: u64,
        duration: Duration,
    },
    SyncFetchStarted {
        repo_full_name: String,
    },
    SyncFetchCompleted {
        repo_full_name: String,
        new_commits: u32,
        bytes_fetched: u64,
        duration: Duration,
    },
    SyncRepoFailed {
        repo_full_name: String,
        error: String,
    },
    SyncRepoUpToDate {
        repo_full_name: String,
    },

    // ── Sync Lifecycle ──
    SyncAllCompleted {
        summary: SyncSummary,
    },
    SyncPaused,
    SyncResumed,
    SyncCancelled,

    // ── Stats ──
    StatsRefreshed {
        total_repos: usize,
        up_to_date: usize,
        behind: usize,
        errors: usize,
        total_size_bytes: u64,
        attention_list: Vec<AttentionItem>,
        size_distribution: Vec<SizeBucket>,
    },

    // ── Errors (unexpected) ──
    FatalError {
        message: String,
    },
}
```

#### Effect
Side effects returned by the reducer. Spawned as Tokio tasks by the main loop:

```
enum Effect {
    // Auth
    AuthenticatePat { token: String },
    StartOAuthDeviceFlow,
    PollOAuthToken { device_code: String, interval: Duration },
    LoadStoredCredentials,

    // Discovery
    DiscoverRepos { token: String, source: RepoSource },

    // Sync (bulk)
    StartSync {
        repos: Vec<RepoInfo>,
        base_path: PathBuf,
        concurrency: usize,
        protocol: CloneProtocol,
    },

    // Sync (single — for retry/inspect flows)
    CloneSingle { repo: RepoInfo, base_path: PathBuf },
    SyncSingle { repo: RepoInfo, local_path: PathBuf },

    // Dashboard
    RefreshDashboardStats,

    // Activity
    RecordSyncRun { summary: SyncSummary },
    ExportRun { run_id: i64, format: ExportFormat, path: PathBuf },

    // Settings
    SaveSettings { settings: Settings },
    TestConnection { token: String },

    // Persistence
    PersistRepoSelection { selected_ids: Vec<i64> },
    LoadPersistedState,

    // No side effect
    None,
}
```

### EventBus Struct

The `EventBus` is the central plumbing that connects background tasks to the main loop:

```
struct EventBus {
    /// Background tasks send AppEvents here
    app_tx: UnboundedSender<AppEvent>,
    /// Main loop drains AppEvents here
    app_rx: UnboundedReceiver<AppEvent>,

    /// Sync engine sends progress events here
    progress_tx: UnboundedSender<AppEvent>,
    /// Main loop drains progress here (merged with app_rx)
    progress_rx: UnboundedReceiver<AppEvent>,

    /// Cancellation signal. Dropping the sender or sending true cancels
    /// all background tasks that hold a receiver.
    cancel_tx: watch::Sender<bool>,
    cancel_rx: watch::Receiver<bool>,
}
```

**Channel types and rationale:**
- `UnboundedSender/Receiver` for `AppEvent` — events are small and bursty. Bounded channels risk blocking background tasks during progress storms. Backpressure is handled by the main loop draining at 60fps.
- `watch::Sender/Receiver` for cancellation — single-producer, multi-consumer. All background tasks clone the receiver and check it periodically. The main loop holds the sender and drops it (or sends `true`) to cancel all tasks.

**Background task cancellation pattern:**
```
// Every background task does this at key yield points:
if *cancel_rx.borrow() {
    return; // Task cancelled
}

// The main loop cancels by:
cancel_tx.send(true)?; // or: drop(cancel_tx);
```

---

## 4. State Management

### App — The Root State

```
struct App {
    state: AppState,
    should_quit: bool,
    terminal_size: (u16, u16),
    tick_count: u64,
    config: AppConfig,
}
```

**Ownership rule:** Only `app::reduce()` holds `&mut App`. The UI layer receives `&App` for rendering. Background tasks receive cloned data, never references to App.

### AppState — Top-Level State Machine

```
enum AppState {
    Auth(AuthScreenState),
    Running(RunningAppState),
}
```

**Transitions:**
- `Auth` → `Running`: Triggered by `AuthSucceeded` event. Persisted credentials skip Auth entirely.
- `Running` → `Auth`: Triggered by `AuthFailed` from a background task (expired token), or user manually switching accounts via `:auth switch`.

### RunningAppState

```
struct RunningAppState {
    active_tab: Tab,
    tabs: TabStates,
    modal_stack: Vec<Modal>,
    command_palette: Option<CommandPaletteState>,
    title_stats: TitleBarStats,
    breadcrumbs: Vec<String>,
    repos: Vec<RepoEntry>,       // master list — merged GitHub + local state
    selected_indices: Vec<usize>, // indices into repos that are selected
    authenticated_user: String,
    services: Arc<Services>,     // read-only handle for effect spawning
}
```

### Tab Enum

```
enum Tab {
    Dashboard = 0,
    Explorer = 1,
    SyncCenter = 2,
    ActivityLog = 3,
    Settings = 4,
}
```

Active tab determines which `TabStates` variant is rendered and which input mappings are active.

### Per-Tab States

Each tab owns its view-specific state. This prevents cross-tab state leakage:

```
struct DashboardTabState {
    attention_list: Vec<AttentionItem>,
    sync_health: SyncHealth,
    size_distribution: Vec<SizeBucket>,
    stats_loading: bool,
    scroll_offset: usize,
}

struct ExplorerTabState {
    source: RepoSource,          // "my_repos", "org:acme", etc.
    org_input: String,           // if source is OrgRepos
    filter_text: String,
    filter_regex: bool,
    skip_archived: bool,
    skip_forks: bool,
    topic_filter: String,
    sort_column: u8,
    sort_ascending: bool,
    mark_mode: bool,
    mark_start: Option<usize>,
    table_scroll: usize,
    detail_scroll: usize,
    loading: bool,
}

struct SyncCenterTabState {
    phase: SyncPhase,
    sync_options: SyncOptions,
    live_log: Vec<LogEntry>,     // ring buffer, max 500 entries
    log_filter: LogFilter,
    log_follow: bool,
    log_scroll: usize,
    overall_progress: Option<OverallProgress>,
}

enum SyncPhase {
    PreSync,                      // confirmation screen
    Active,                       // progress screen
    Paused,                       // paused state
    Completed(SyncSummary),       // results screen
    Cancelled(String),            // reason
}

struct ActivityLogTabState {
    runs: Vec<SyncRunRow>,
    selected_run: Option<usize>,
    run_detail_scroll: usize,
    filter_text: String,
    show_filter: RunFilter,
    loading: bool,
}

struct SettingsTabState {
    category: SettingsCategory,
    editing_field: Option<(usize, String)>, // (field_index, draft_value)
    fields: Vec<SettingsField>,
    has_unsaved_changes: bool,
}
```

### Modal Stack

```
enum Modal {
    Confirmation {
        title: String,
        message: String,
        items: Vec<String>,      // list of affected repos
        confirm_label: String,
        cancel_label: String,
        focus: ModalFocus,       // Confirm or Cancel
        danger: bool,            // red styling for destructive actions
    },
    RepoDetail {
        repo_index: usize,       // index into app.repos
        scroll: usize,
    },
    ErrorDetail {
        repo_full_name: String,
        error_message: String,
        raw_output: String,
        timestamp: DateTime,
    },
    Help {
        scroll: usize,
    },
    InputPrompt {
        title: String,
        prompt: String,
        value: String,
        mask_input: bool,        // for passwords/tokens
        cursor_pos: usize,
    },
}
```

**Modal behavior:**
- `modal_stack` is a `Vec<Modal>`. Only the last element is rendered.
- `Action::DismissModal` pops the stack. If the stack is empty, `Esc` navigates back (breadcrumb).
- Key events are routed to the topmost modal first. If the modal doesn't handle a key, it's ignored (focus trapping).
- Modals render on top of the active tab's content with a dimmed backdrop.

---

## 5. Reducer

The reducer is a pure-ish function (pure in state transition, impure only in that it returns effects):

```
fn reduce(app: &mut App, action: Action) -> Vec<Effect>
fn reduce_event(app: &mut App, event: AppEvent) -> Vec<Effect>
```

**Design contract:**
- `reduce` and `reduce_event` are the **only** functions that mutate `App`.
- They run **synchronously** on the main thread — no `.await`, no blocking I/O.
- They return `Vec<Effect>` for async side effects.
- They never panic on malformed state — they return `Effect::None` for invalid transitions.

### Example: Explorer Start Sync Flow

```
Action::ExplorerStartSync
    → validate: are repos selected?
    → transition: SyncCenterTabState.phase = PreSync(sync_summary)
    → transition: active_tab = Tab::SyncCenter
    → return: [Effect::None]

User presses Enter on Sync Center:
Action::SyncStart
    → transition: SyncCenterTabState.phase = Active
    → return: [Effect::StartSync { repos, base_path, concurrency, protocol }]

Background task emits progress events:
AppEvent::SyncCloneStarted { repo_full_name }
    → update: SyncCenterTabState.live_log (add entry)
    → return: [Effect::None]

AppEvent::SyncCloneProgress { repo_full_name, bytes, total }
    → update: live_log entry for that repo (update in place)
    → update: overall_progress
    → return: [Effect::None]

AppEvent::SyncAllCompleted { summary }
    → transition: SyncCenterTabState.phase = Completed(summary)
    → return: [Effect::RecordSyncRun { summary }, Effect::RefreshDashboardStats]
```

### Example: Auth Flow

```
Action::AuthSubmitToken(token)
    → return: [Effect::AuthenticatePat { token }]

Background task validates:
AppEvent::AuthSucceeded { username, scopes }
    → persist token to keyring (via Effect)
    → transition: AppState::Running (initialize empty tab states)
    → return: [Effect::DiscoverRepos { token, source: All }]

AppEvent::AuthFailed { reason, detail }
    → update: AuthScreenState.error = Some((reason, detail))
    → return: [Effect::None]
```

---

## 6. Dependency Flow

### Compile-Time Dependency Graph

```
main ────────────────────────────────────────────────────────────┐
  │                                                               │
  ├──▶ app ──▶ event                                             │
  │      │                                                        │
  │      └──▶ service ──▶ db     (via trait Database)             │
  │                    ├──▶ github (via trait GithubApi)           │
  │                    ├──▶ git    (via trait GitOps)              │
  │                    └──▶ keyring(via trait KeyringStore)        │
  │                                                               │
  ├──▶ ui ──▶ app (read-only &App reference)                      │
  │      └──▶ event (Action enum for input mapping)               │
  │                                                               │
  └──▶ config                                                    │
```

### Runtime Dependency Injection

At startup, `main.rs` constructs concrete implementations and groups them:

```
struct Services {
    github: Arc<dyn GithubApi>,
    git: Arc<dyn GitOps>,
    db: Arc<dyn Database>,
    keyring: Arc<dyn KeyringStore>,
}
```

`Services` is wrapped in `Arc` and stored in `RunningAppState`. It's passed to effect executors by cloning the `Arc`.

### Infrastructure Traits

```
trait GithubApi: Send + Sync {
    async fn validate_token(&self, token: &str) -> Result<UserInfo>;
    async fn list_my_repos(&self, token: &str) -> Result<Vec<RemoteRepo>>;
    async fn list_org_repos(&self, token: &str, org: &str) -> Result<Vec<RemoteRepo>>;
    async fn list_starred(&self, token: &str) -> Result<Vec<RemoteRepo>>;
    async fn device_flow_init(&self) -> Result<DeviceFlowInit>;
    async fn device_flow_poll(&self, device_code: &str) -> Result<Option<String>>;
}

trait GitOps: Send + Sync {
    async fn clone(&self, url: &str, path: &Path) -> Result<CloneProgress>;
    async fn fetch(&self, path: &Path) -> Result<FetchResult>;
    async fn status(&self, path: &Path) -> Result<LocalRepoStatus>;
    fn is_git_repo(&self, path: &Path) -> bool;
}

trait Database: Send + Sync {
    fn upsert_repos(&self, repos: &[RepoRow]) -> Result<()>;
    fn get_all_repos(&self) -> Result<Vec<RepoRow>>;
    fn update_local_status(&self, full_name: &str, status: &LocalStatus) -> Result<()>;
    fn insert_sync_run(&self, run: &SyncRunRow) -> Result<i64>;
    fn insert_sync_entries(&self, entries: &[SyncEntryRow]) -> Result<()>;
    fn get_sync_runs(&self, limit: usize) -> Result<Vec<SyncRunRow>>;
    fn get_sync_entries(&self, run_id: i64) -> Result<Vec<SyncEntryRow>>;
    fn get_setting(&self, key: &str) -> Result<Option<String>>;
    fn set_setting(&self, key: &str, value: &str) -> Result<()>;
}

trait KeyringStore: Send + Sync {
    fn set_token(&self, token: &str) -> Result<()>;
    fn get_token(&self) -> Result<Option<String>>;
    fn delete_token(&self) -> Result<()>;
}
```

**Why traits at infrastructure boundaries:**
1. **Testing:** All service-layer functions can be unit-tested with mock implementations.
2. **Multi-provider future:** `GithubApi` becomes `ScmApi` when GitLab support lands — the trait boundary already exists.
3. **Platform abstraction:** `KeyringStore` hides OS keyring differences. `Database` hides SQLite vs. future storage backends.
4. **Deterministic tests:** Mock `GitOps` means sync engine tests don't need actual git repos.

---

## 7. Main Loop

```
async fn main() -> Result<()> {
    // 1. Parse config
    let config = config::parse();

    // 2. Initialize infrastructure
    let db = db::SqliteDatabase::open(&config.db_path)?;
    db.run_migrations()?;
    let github = github::OctocrabGithubApi::new(config.user_agent.clone());
    let git = git::Git2GitOps::new(config.git_binary.clone());
    let keyring = keyring::PlatformKeyring::new("git-vacuum")?;
    let services = Arc::new(Services { github, git, db, keyring });

    // 3. Initialize terminal
    let mut terminal = ui::terminal::init()?;

    // 4. Create event bus
    let (app_tx, app_rx) = tokio::sync::mpsc::unbounded_channel();
    let (progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let event_bus = EventBus { app_tx, app_rx, progress_tx, progress_rx, cancel_tx, cancel_rx };

    // 5. Initialize app state
    let mut app = App::new(config, services, event_bus);

    // 6. Try loading stored credentials
    app.reduce(Action::LoadStoredCredentials); // may transition to Running

    // 7. Main loop
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(16); // ~60fps

    loop {
        // ── Poll terminal input ──
        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    let action = ui::input::map_key_to_action(key, &app);
                    let effects = app.reduce(action);
                    execute_effects(effects, &app);
                }
                Event::Resize(w, h) => {
                    app.terminal_size = (w, h);
                }
                _ => {}
            }
        }

        // ── Tick for animations ──
        let now = Instant::now();
        if now - last_tick >= tick_rate {
            app.tick_count += 1;
            last_tick = now;
        }

        // ── Drain AppEvents ──
        while let Ok(event) = app.event_bus.app_rx.try_recv() {
            let effects = app.reduce_event(event);
            execute_effects(effects, &app);
        }
        while let Ok(event) = app.event_bus.progress_rx.try_recv() {
            let effects = app.reduce_event(event);
            execute_effects(effects, &app);
        }

        // ── Render ──
        terminal.draw(|frame| {
            ui::render(frame, &app);
        })?;

        // ── Check quit ──
        if app.should_quit {
            break;
        }
    }

    // 8. Cleanup
    ui::terminal::restore()?;
    Ok(())
}
```

### Effect Execution

`execute_effects` spawns each effect as a Tokio task. Each task has access to `Arc<Services>` and a clone of the event bus channels:

```
fn execute_effects(effects: Vec<Effect>, app: &App) {
    for effect in effects {
        match effect {
            Effect::AuthenticatePat { token } => {
                let github = app.services.github.clone();
                let keyring = app.services.keyring.clone();
                let app_tx = app.event_bus.app_tx.clone();

                tokio::spawn(async move {
                    match github.validate_token(&token).await {
                        Ok(user) => {
                            let _ = keyring.set_token(&token);
                            let _ = app_tx.send(AppEvent::AuthSucceeded {
                                username: user.login,
                                scopes: user.scopes,
                                token_expires: user.expires,
                            });
                        }
                        Err(e) => {
                            let _ = app_tx.send(AppEvent::AuthFailed {
                                reason: e.kind.to_string(),
                                detail: e.to_string(),
                            });
                        }
                    }
                });
            }
            Effect::StartSync { repos, base_path, concurrency, protocol } => {
                let git = app.services.git.clone();
                let progress_tx = app.event_bus.progress_tx.clone();
                let cancel_rx = app.event_bus.cancel_rx.clone();
                let app_tx = app.event_bus.app_tx.clone();

                tokio::spawn(async move {
                    service::sync_engine::run_sync(
                        repos, base_path, concurrency, protocol,
                        git, progress_tx, app_tx, cancel_rx,
                    ).await;
                });
            }
            Effect::None => {}
            // ... other effects
        }
    }
}
```

**Key constraint:** Effect tasks are fire-and-forget from the main loop's perspective. They do not hold references to `App`. They communicate exclusively through the event bus channels. This prevents aliased mutable state.

---

## 8. Background Jobs

### 8.1 Sync Engine (`service/sync_engine.rs`)

The most complex background system. Orchestrates concurrent clone and sync operations.

**Architecture:**
- A `Semaphore` with `concurrency` permits limits simultaneous git operations.
- For each repo, a task acquires a permit, performs the operation, and releases it.
- Progress is streamed via `progress_tx` at key milestones.
- A coordinator task waits for all repo tasks to complete (or cancellation), then emits `AppEvent::SyncAllCompleted`.

**Flow:**
```
run_sync(repos, base_path, concurrency, protocol, git, progress_tx, app_tx, cancel_rx)
  │
  ├── Determine operation per repo:
  │     if local_path exists && is_git_repo → sync (fetch + fast-forward)
  │     else → clone
  │
  ├── Create semaphore with `concurrency` permits
  │
  ├── For each repo:
  │     spawn(async {
  │         acquire permit
  │         if operation == Clone:
  │             emit SyncCloneStarted
  │             git.clone(url, path, progress_callback)
  │               → callback emits SyncCloneProgress every ~100ms
  │             emit SyncCloneCompleted
  │         else:
  │             emit SyncFetchStarted
  │             git.fetch(path)
  │             git.status(path) → check behind_count
  │             if behind_count > 0: git fast-forward
  │             emit SyncFetchCompleted
  │         release permit
  │     })
  │
  ├── Join all repo tasks (select! with cancel_rx)
  │
  └── Emit SyncAllCompleted { summary }
```

**Progress callback chain (for clone progress):**
```
git2::Remote::fetch() with transfer_progress callback
  → mpsc::Sender::send(CloneProgress { bytes, total })
    → service::sync_engine converts to AppEvent::SyncCloneProgress
      → progress_tx.send(event)
        → main loop drains → reduce_event → updates live_log entry
```

**Concurrency considerations:**
- Default concurrency: 8 (override in settings, 1-50 range)
- Disk I/O is the bottleneck, not network. git2-rs clone operations are synchronous (they call libgit2 C functions). These must run on a blocking thread pool (`tokio::task::spawn_blocking`), not on async tasks.
- Network I/O (git fetch HTTPS/SSH) happens inside libgit2. Using `spawn_blocking` avoids blocking the async runtime.

**Pause/Resume/Cancel:**
- `cancel_rx` is checked before starting each new repo operation. On cancel, pending tasks are dropped immediately. Active operations complete.
- Pause sets an `AtomicBool` that all tasks check before starting their next op. Active operations complete.
- Resume clears the flag and notifies waiting tasks.

### 8.2 Discovery (`service/discovery.rs`)

**Flow:**
```
discover_repos(token, source, github, db)
  │
  ├── Call GitHub API based on source:
  │     MyRepos → github.list_my_repos(token)    // paginated
  │     Org(name) → github.list_org_repos(token, name)
  │     Starred → github.list_starred(token)
  │     All → merge(MyRepos, MemberOrgs)
  │
  ├── Get cached repo data from SQLite
  │
  ├── Merge: for each remote repo, check DB for:
  │     - clone_status (not_cloned, cloned, stale, error)
  │     - local_path
  │     - last_synced_at
  │
  ├── For each cloned repo, optionally check local filesystem:
  │     - Does the directory exist?
  │     - git.status() → behind_count
  │
  ├── Upsert merged data to SQLite
  │
  └── Emit ReposDiscovered { repos: Vec<RepoEntry> }
```

**Local filesystem check optimization:** Only check filesystem for repos marked as "cloned" in DB. For 100+ repos, scanning the filesystem is expensive. The DB cache is the primary source. Filesystem checks happen asynchronously (via `Effect::RefreshDashboardStats`) and update the DB progressively.

### 8.3 Stats Refresh (`service/stats.rs`)

Triggered by `Effect::RefreshDashboardStats`. Runs in background, updates DB, emits `AppEvent::StatsRefreshed`.

**What it computes:**
- Total repos, up-to-date count, behind count, error count
- Total on-disk size (sum of `local_size_kb` from DB + dir walk for unmeasured repos)
- Attention list: top 10 repos that are behind or have errors, sorted by urgency
- Size distribution: bucket counts for histogram

### 8.4 Auth Flow (`service/auth_service.rs`)

**PAT flow (sync validation, async task):**
```
Effect::AuthenticatePat { token }
  → github.validate_token(token)
    → /user + /user/repos?per_page=1
  → on success: keyring.set_token(token), emit AuthSucceeded
  → on failure: emit AuthFailed
```

**OAuth device flow (multi-step async):**
```
Effect::StartOAuthDeviceFlow
  → github.device_flow_init()
    → POST /login/device/code
  → emit OAuthCodeReceived { user_code, verification_uri, expires_in }

Effect::PollOAuthToken { device_code, interval }
  → loop every `interval` seconds:
    → github.device_flow_poll(device_code)
      → POST /login/oauth/access_token
    → if granted: emit OAuthTokenReceived { token }
    → if pending: continue
    → if expired/denied: emit OAuthTimeout or AuthFailed
    → break on cancel_rx changed
```

---

## 9. Data Flow Summary

A full sync operation traced through the system:

```
USER presses Space on 5 repos, then Enter
  │
  ├── ui/input.rs maps Enter → Action::ExplorerStartSync
  │
  ├── app::reduce(Action::ExplorerStartSync)
  │     → validates: at least one repo selected
  │     → computes sync summary (3 new clones, 2 syncs)
  │     → sets SyncCenter.phase = PreSync
  │     → sets active_tab = SyncCenter
  │     → returns [Effect::None]
  │
  ├── ui::render() shows the PreSync confirmation screen
  │
USER presses Enter on "Start Sync"
  │
  ├── ui/input.rs maps Enter → Action::SyncStart
  │
  ├── app::reduce(Action::SyncStart)
  │     → sets SyncCenter.phase = Active
  │     → returns [Effect::StartSync { repos, ... }]
  │
  ├── execute_effects spawns Tokio task:
  │     service::sync_engine::run_sync(...)
  │
  ├── sync_engine:
  │     ├── for each of 5 repos:
  │     │     ├── acquire semaphore permit
  │     │     ├── emit SyncCloneStarted / SyncFetchStarted via progress_tx
  │     │     ├── git2::clone() or git2::fetch() via spawn_blocking
  │     │     ├── emit SyncCloneProgress (every 100ms) via progress_tx
  │     │     └── emit SyncCloneCompleted / SyncFetchCompleted via progress_tx
  │     └── emit SyncAllCompleted via app_tx
  │
  ├── Main loop drains progress_tx + app_tx (interleaved):
  │     AppEvent::SyncCloneStarted   → add entry to live_log
  │     AppEvent::SyncCloneProgress  → update entry in live_log (bytes/total)
  │     AppEvent::SyncCloneCompleted → mark entry ✓, update overall_progress
  │     AppEvent::SyncAllCompleted    → set phase = Completed(summary)
  │                                   → return [Effect::RecordSyncRun, Effect::RefreshStats]
  │
  ├── execute_effects:
  │     Effect::RecordSyncRun → db.insert_sync_run() + db.insert_sync_entries()
  │     Effect::RefreshDashboardStats → stats computation → emit StatsRefreshed
  │
  ├── ui::render(60fps throughout):
  │     Shows live progress bar, per-repo log updating in place, throughput
  │
  └── Final state: SyncCenter shows results screen, Dashboard updated with new stats
```

---

## 10. SQLite Schema

```
-- Schema version tracking
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Repository catalog (remote data cached from GitHub + local state)
CREATE TABLE repos (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    github_id       INTEGER NOT NULL UNIQUE,
    owner           TEXT NOT NULL,
    name            TEXT NOT NULL,
    full_name       TEXT NOT NULL UNIQUE,
    description     TEXT,
    language        TEXT,
    stars           INTEGER NOT NULL DEFAULT 0,
    default_branch  TEXT NOT NULL DEFAULT 'main',
    visibility      TEXT NOT NULL CHECK(visibility IN ('public','private','internal')),
    is_fork         INTEGER NOT NULL DEFAULT 0,
    is_archived     INTEGER NOT NULL DEFAULT 0,
    clone_url_ssh   TEXT,
    clone_url_https TEXT,
    size_kb         INTEGER,
    pushed_at       TEXT,
    created_at      TEXT,
    updated_at      TEXT,

    -- Local tracking
    clone_status    TEXT NOT NULL DEFAULT 'not_cloned'
                        CHECK(clone_status IN ('not_cloned','cloned','stale','error')),
    local_path      TEXT,
    local_size_kb   INTEGER,
    last_synced_at  TEXT,
    last_error      TEXT,
    behind_count    INTEGER NOT NULL DEFAULT 0,

    -- UI preferences
    selected        INTEGER NOT NULL DEFAULT 1,
    discovered_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_repos_full_name ON repos(full_name);
CREATE INDEX idx_repos_owner ON repos(owner);
CREATE INDEX idx_repos_clone_status ON repos(clone_status);

-- Sync run history (one row per sync operation)
CREATE TABLE sync_runs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at      TEXT NOT NULL,
    completed_at    TEXT,
    status          TEXT NOT NULL DEFAULT 'running'
                        CHECK(status IN ('running','completed','cancelled','failed')),
    trigger         TEXT NOT NULL DEFAULT 'manual'
                        CHECK(trigger IN ('manual','scheduled','cli')),
    total_repos     INTEGER NOT NULL DEFAULT 0,
    cloned_count    INTEGER NOT NULL DEFAULT 0,
    updated_count   INTEGER NOT NULL DEFAULT 0,
    failed_count    INTEGER NOT NULL DEFAULT 0,
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    options_json    TEXT
);

CREATE INDEX idx_sync_runs_started ON sync_runs(started_at DESC);

-- Per-repo results within a sync run
CREATE TABLE sync_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    repo_id         INTEGER NOT NULL REFERENCES repos(id),
    operation       TEXT NOT NULL CHECK(operation IN ('clone','sync','skip')),
    status          TEXT NOT NULL CHECK(status IN ('running','success','failed')),
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    new_commits     INTEGER NOT NULL DEFAULT 0,
    duration_ms     INTEGER,
    error_message   TEXT
);

CREATE INDEX idx_sync_entries_run ON sync_entries(run_id);

-- Application settings (key-value)
CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed defaults
INSERT OR IGNORE INTO settings (key, value) VALUES
    ('clone_path', ''),
    ('default_concurrency', '8'),
    ('default_protocol', 'ssh'),
    ('skip_archived_default', 'true'),
    ('skip_forks_default', 'true'),
    ('auto_prune', 'false'),
    ('include_wikis', 'false'),
    ('lfs_enabled', 'false');
```

**Schema design rationale:**
- `repos` is the central table. It stores both GitHub API data (cached) and local state. This avoids joins for the most common query: "show all repos with their clone status."
- `sync_runs` + `sync_entries` form a standard parent-child relationship for activity log. `ON DELETE CASCADE` keeps things clean.
- `settings` is a simple key-value store. No need for typed columns — settings are validated at the application layer.

---

## 11. Key Architectural Decisions (with Rationale)

### Decision 1: Unidirectional data flow over MVC
**Chosen:** Elm/Redux-style reducer pattern  
**Rejected:** MVC with shared mutable state  
**Why:** TUI apps with concurrent background operations create complex state transitions. A single reducer eliminates race conditions and makes state transitions testable as pure functions. The `Action → State → Effect → Event → State` loop is easy to trace and debug.

### Decision 2: Effect-based side effects over async reducers
**Chosen:** Reducer returns `Vec<Effect>`; main loop spawns them  
**Rejected:** Async reducer with direct `.await`  
**Why:** Keeping the reducer synchronous guarantees it never blocks the render loop. At 60fps, every millisecond counts. Effect spawning gives us concurrency without coupling the reducer to Tokio.

### Decision 3: Trait abstraction at all infrastructure boundaries
**Chosen:** `GithubApi`, `GitOps`, `Database`, `KeyringStore` traits  
**Rejected:** Direct use of Octocrab, git2-rs, rusqlite throughout  
**Why:** The product spec mandates multi-provider support in the future. Traits at the infrastructure layer now prevent a rewrite later. They also enable deterministic testing — the sync engine can be tested with a mock `GitOps` that doesn't touch the filesystem.

### Decision 4: SQLite as single source of truth for repo state
**Chosen:** All repo metadata cached in SQLite; filesystem checked asynchronously  
**Rejected:** Filesystem-only state (walking `~/git-vacuum/` on every refresh)  
**Why:** Walking 100+ directories and running `git status` on each is slow. Caching in SQLite gives instant dashboard loads. The async stats refresh keeps the cache accurate without blocking the UI.

### Decision 5: Unbounded channels for events
**Chosen:** `tokio::sync::mpsc::unbounded_channel()` for event bus  
**Rejected:** Bounded channels with backpressure  
**Why:** Progress events during a sync can be bursty (100+ repos × per-second progress updates). Bounded channels risk dropping events or blocking background tasks. The main loop drains channels at 60fps — if it can't keep up, the terminal is too slow anyway. The ring buffer in `live_log` (max 500 entries) provides natural memory bounds.

### Decision 6: Multi-crate vs single-crate
**Chosen:** Single crate with strict module boundaries  
**Rejected:** Workspace with separate crates for `db`, `github`, `git`, `ui`  
**Why:** For an MVP, a single crate reduces build complexity and compilation time. Module visibility (`pub(crate)`) enforces boundaries. The trait-based architecture makes extraction into separate crates trivial later if needed.

### Decision 7: `spawn_blocking` for git2-rs operations
**Chosen:** All git2-rs calls run via `tokio::task::spawn_blocking`  
**Rejected:** Calling git2-rs directly from async tasks  
**Why:** git2-rs (libgit2) performs synchronous filesystem and network I/O in C. Calling it from an async task blocks the Tokio worker thread, starving other async tasks. `spawn_blocking` moves these operations to a dedicated thread pool, keeping the async runtime responsive.

---

## 12. Startup Sequence

```
1. Parse CLI args (clap)
   └── token, path, concurrency, protocol overrides

2. Open/create SQLite database
   └── Run migrations if needed

3. Initialize infrastructure services
   ├── GithubApi (Octocrab)
   ├── GitOps (git2-rs)
   └── KeyringStore (platform keyring)

4. Initialize terminal (crossterm)
   ├── Enable raw mode
   ├── Enter alternate screen
   └── Hide cursor

5. Create EventBus channels

6. Create App state
   ├── Load settings from SQLite
   ├── Check for stored credentials in keyring
   │   ├── Found → validate in background → transition to Running
   │   └── Not found → transition to Auth screen

7. Enter main loop

8. On Quit:
   ├── Cancel all background tasks (cancel_tx.send(true))
   ├── Drain remaining events (brief timeout)
   ├── Restore terminal
   └── Exit
```

---

## 13. Error Handling Strategy

**Layer-specific error handling:**

| Layer | Strategy |
|-------|----------|
| **Infrastructure** | Return `Result<T, Error>` with typed errors (thiserror). Never panic. |
| **Service** | Convert infrastructure errors to `AppEvent` variants. Log details, emit user-facing messages. |
| **Application** | Never panic. Invalid state transitions return `Effect::None`. |
| **Presentation** | Defensive rendering. If state is inconsistent, render what we can + error banner. |

**Error propagation pattern:**
```
// In service layer:
match github.list_my_repos(&token).await {
    Ok(repos) => emit ReposDiscovered { repos },
    Err(e) if e.is_rate_limited() => emit AppEvent::DiscoveryFailed {
        error: "GitHub API rate limit reached. Try again later.".into()
    },
    Err(e) if e.is_auth_error() => emit AppEvent::AuthFailed { ... }, // triggers re-auth
    Err(e) => {
        log::error!("Discovery failed: {:?}", e);
        emit AppEvent::DiscoveryFailed { error: e.to_string() }
    }
}
```

**Fatal errors:** If a truly unrecoverable error occurs (disk failure, database corruption), the app shows a full-screen error with the option to quit. This is handled by `AppEvent::FatalError` → `AppState::FatalError` → `ui::screens::fatal_error::render()`.

---

## 14. Testing Strategy

**Unit tests:**
- `app::reduce()` — pure function, test with constructed states: "given this state and this action, assert the new state and effects"
- `service::sync_engine` — with mock `GitOps` and `Database`, verify concurrency, pause/resume, cancellation
- `service::discovery` — with mock `GithubApi`, verify merging logic (new repos, deleted repos, updated repos)
- `ui::input::map_key_to_action()` — verify correct Action for each key in each context

**Integration tests:**
- In-memory SQLite database for full service-layer tests
- Mock GitHub API (returns known fixture data) for discovery tests
- Temp directories for git2-rs integration tests

**Snapshot tests (future):**
- Render each screen state to a string buffer and compare against saved snapshots
- Catches regressions in UI layout
