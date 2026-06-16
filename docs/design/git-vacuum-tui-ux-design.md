# Git-Vacuum — Terminal UX Design

**Inspired by:** lazygit (panel navigation, contextual actions, help overlay) + k9s (command palette, marking, breadcrumbs, skins)  
**Framework:** Ratatui  
**Interaction model:** Keyboard-first, every action has a visible shortcut

---

## 1. Main Layout (Fixed Shell)

Every screen shares this outer structure. Only the **Main Content** area changes per tab.

```
┌─ Title Bar ─────────────────────────────────────────────────────────────────────┐
│ ▓▓ git-vacuum ▓▓   user: prakash   org: acme-corp   128 repos · 31.5 GB local   │
├─ Tab Bar ───────────────────────────────────────────────────────────────────────┤
│  ▸ Dashboard    Explorer    Sync Center    Activity Log    Settings              │
├─ Main Content ──────────────────────────────────────────────────────────────────┤
│                                                                                  │
│                             (tab-specific content)                               │
│                                                                                  │
│                                                                                  │
├─ Breadcrumb ────────────────────────────────────────────────────────────────────┤
│  Settings ▸ Sync Preferences                                                     │
├─ Key Bar ───────────────────────────────────────────────────────────────────────┤
│  1:Dash  2:Explore  3:Sync  4:Log  5:Settings  Tab:Next  q:Quit  ?:Help  ::Cmd  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Layout rules:**
- Title Bar: 2 rows. Row 1 = brand + user/org info. Row 2 = stats summary.
- Tab Bar: 1 row. Active tab highlighted with reverse video. Inactive tabs dimmed.
- Main Content: Remaining height minus breadcrumb and key bar.
- Breadcrumb: 1 row. Shows navigation path. Only visible after drilling into sub-views.
- Key Bar: 1 row. Dynamic — changes per tab. First 4–6 slots show tab-specific keys. Last 4 slots are global (`q`, `?`, `Tab`, `:`).

**Ratatui components used:**
- `Layout` with `Constraint::Length` for fixed bars, `Constraint::Min` for main content
- `Paragraph` for title, breadcrumb, key bar
- `Tabs` widget for tab bar

---

## 2. Dashboard Screen (Tab 1 — Default)

**Purpose:** At-a-glance health of all local repositories. The "home screen."

```
┌─ Dashboard ──────────────────────────────────────────────────────────────────────┐
│                                                                                  │
│  ┌─ Sync Health ──────────────────────┐  ┌─ Quick Stats ───────────────────────┐ │
│  │                                    │  │                                      │ │
│  │   ████████████████░░░░  88%       │  │  Total repos:          128           │ │
│  │   112 up to date                   │  │  On disk:             31.5 GB       │ │
│  │                                    │  │  Last full sync:      2h ago        │ │
│  │   ██████░░░░░░░░░░░░░░  12%       │  │  New commits avail:   47 repos      │ │
│  │   16 behind remote                 │  │  Errors:              3 repos       │ │
│  │                                    │  │                                      │ │
│  └────────────────────────────────────┘  └──────────────────────────────────────┘ │
│                                                                                  │
│  ┌─ Repos Needing Attention ────────────────────────────────────────────────────┐ │
│  │                                                                              │ │
│  │   ↑  acme/web-frontend       +14 commits behind   (org)     · 2h ago         │ │
│  │   ↑  acme/api-gateway        +3 commits behind     (org)     · 1d ago        │ │
│  │   ✗  prakash/old-project     404 repo deleted      (user)    · 6h ago        │ │
│  │   ✗  acme/legacy-db          clone failed: auth    (org)     · 1d ago        │ │
│  │   ↑  prakash/dotfiles         +7 commits behind     (user)    · 30m ago      │ │
│  │   ... (scrollable)                                                           │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│  ┌─ Repo Size Distribution ─────────────────────────────────────────────────────┐ │
│  │  <1MB     ████████████████████████████████████████  62                        │ │
│  │  1-10MB   ████████████████████████████              35                        │ │
│  │  10-100MB ██████████████████                        22                        │ │
│  │  100MB-1G ██████                                     7                        │ │
│  │  >1GB     ██                                          2                        │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Layout:** 2×2 grid in main area. Top-left: Sync health gauge. Top-right: Quick stats. Bottom-left: Attention-needed list (scrollable, takes 2 rows of height). Bottom-right: Size distribution bar chart.

**Status icons in Attention list:**
- `↑` Yellow — Behind remote (stale)
- `✗` Red — Error state  
- `⏳` Cyan — Sync in progress (if navigated here during a sync)
- `✓` Green — Just updated (transient, fades after 5s)

**Key Bar (Dashboard):**
```
 s:Start Sync  r:Refresh  Enter:Inspect Repo  ↑↓:Navigate  Tab:Next  q:Quit  ?:Help
```

**Interactions:**
- `Enter` on a repo in the Attention list → opens a detail popup for that repo
- `s` → jumps to Sync Center tab and begins syncing all stale repos
- `r` → re-queries GitHub API to refresh stats

---

## 3. Repository Explorer (Tab 2)

**Purpose:** Browse, filter, and select repositories. The primary interaction surface.

```
┌─ Explorer ───────────────────────────────────────────────────────────────────────┐
│  Source: [▸ My Repos] [ Org Repos ▾ acme-corp ] [ Starred ] [ All ]              │
│  Filter: /node                                                       [X] Clear   │
│  ┌─ Toolbar ────────────────────────────────────────────────────────────────────┐ │
│  │ [✓] Skip archived  [✓] Skip forks  [ ] Regex: ______  [ ] Topic: ________   │ │
│  │ 47 / 128 repos selected                               Select: All | None      │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│  ┌─ Repo Table ─────────────────────────────┬─ Detail Panel ────────────────────┐ │
│  │ #   Name             Owner     Vis   Size│                                    │ │
│  │─── ──────────────── ──────── ───── ─────│  acme/web-frontend                 │ │
│  │ ▸✓  web-frontend     acme      pub   2.3M│  ───────────────────────────────── │ │
│  │  ✓  api-gateway      acme      priv  8.1M│  Language:    TypeScript           │ │
│  │  ✓  auth-service     acme      priv  1.2M│  Stars:       247                  │ │
│  │  ✓  data-pipeline    acme      pub  45.0M│  Default:     main                 │ │
│  │     legacy-auth      acme      priv  0.5M│  Last push:   2026-06-15           │ │
│  │     mobile-app       acme      priv 120.0M│  Clone URL:   git@github.com:acme… │ │
│  │     docs-site        acme      pub   3.7M│  License:     MIT                  │ │
│  │     terraform-infra  acme      priv 15.2M│  Local path:  ~/git-vacuum/acme/…  │ │
│  │     ...                                    │                                    │ │
│  │                                           │  ┌─ Local Status ────────────────┐ │ │
│  │                                           │  │ ✓ Cloned · 2026-06-14         │ │ │
│  │                                           │  │ ↑ 14 commits behind remote    │ │ │
│  │                                           │  │ Size on disk: 23.7 MB         │ │ │
│  │                                           │  └───────────────────────────────┘ │ │
│  │                                           │                                    │ │
│  │   47 selected · 45 cloned · 2 pending     │  Actions: [Clone] [Force Sync]     │ │
│  └───────────────────────────────────────────┴────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

**Layout:** Source selector + filter bar at top. Toolbar below. 70/30 split: repo table (left) + detail panel (right). Detail panel shows selected repo info + local status + action buttons.

**Table columns (sortable by key):**
| Key | Column | Description |
|-----|--------|-------------|
| `1` | `[✓]` | Selection checkbox (Space toggles) |
| `2` | Name | Repository name |
| `3` | Owner | GitHub owner (user or org) |
| `4` | Vis | Visibility: `pub`, `priv`, `int` (internal) |
| `5` | Size | Human-readable (KB/MB/GB) |
| `6` | Status | ✓ cloned, ↑ stale, ✗ error, — not cloned |

**Color coding for rows:**
- White/default — not selected, not filtered
- Green highlight — selected (checkbox ✓)
- Dim/gray — filtered out by toolbar toggles
- Yellow — stale (local exists but behind remote)
- Red — last sync errored
- Blue cursor line — current row

**Selection mechanics:**
- `Space` — toggle current row
- `Ctrl+A` — select all visible
- `Ctrl+D` — deselect all
- `v` — enter visual mark mode (hold Shift+↓/↑ to range-select, like lazygit)
- `m` — mark current; `M` — unmark all

**Key Bar (Explorer):**
```
 Space:Toggle  v:Mark Mode  Ctrl+A:All  Ctrl+D:None  /:Filter  1-6:Sort  Enter:Clone Selected  Tab:Next  ::Cmd
```

**Filter bar behavior:**
- `/` focuses the filter input
- Type to live-filter the table (substring match on repo name + owner)
- `Escape` clears filter and returns focus to table
- Filter supports regex when toggled in toolbar

---

## 4. Sync Center (Tab 3)

**Purpose:** Execute and monitor clone/sync operations with live progress.

### 4a. Pre-Sync: Confirmation View

Shown when user arrives with selected repos but hasn't started sync yet.

```
┌─ Sync Center ────────────────────────────────────────────────────────────────────┐
│                                                                                  │
│  ┌─ Sync Summary ───────────────────────────────────────────────────────────────┐ │
│  │                                                                              │ │
│  │   Repos to clone:     12  (324 MB estimated)                                 │ │
│  │   Repos to update:    35  (new commits detected)                              │ │
│  │   ─────────────────────────────────────                                       │ │
│  │   Total operations:   47                                                     │ │
│  │   Destination:        ~/git-vacuum/                                          │ │
│  │   Concurrency:        8 workers                                               │ │
│  │   Protocol:           SSH                                                     │ │
│  │                                                                              │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│  ┌─ Options ────────────────────────────────────────────────────────────────────┐ │
│  │  [ ] Include wikis         [ ] Fetch LFS objects                             │ │
│  │  [ ] Mirror mode (bare)    [ ] Prune deleted repos after sync                │ │
│  │  Concurrency: [8 ▾]        Protocol: [SSH ▾]                                 │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│                           ┌──────────────┐                                       │
│                           │ ▶  Start Sync │                                       │
│                           └──────────────┘                                       │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 4b. Active Sync: Progress View

```
┌─ Sync Center ────────────────────────────────────────────────────────────────────┐
│                                                                                  │
│  ┌─ Overall Progress ───────────────────────────────────────────────────────────┐ │
│  │                                                                              │ │
│  │   ████████████████░░░░░░░░░░░░░░░░░░░░░░  34 / 47 repos                      │ │
│  │                                                                              │ │
│  │   ✓ 22 cloned    ↑ 9 updated    ⣾ 3 active    — 13 queued    ✗ 0 failed     │ │
│  │                                                                              │ │
│  │   Data:  1.2 GB / 3.8 GB   │   Throughput: 42.3 MB/s   │   8 workers active  │ │
│  │   Elapsed: 00:02:47         │   Est. remaining: 00:01:05                      │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│  ┌─ Live Log ───────────────────────────────────────────────────────────────────┐ │
│  │                                                                              │ │
│  │   ✓  acme/web-frontend        cloned        (2.3 MB,    0:03)                │ │
│  │   ✓  acme/api-gateway         fetched +14   (8.1 MB,    0:08)                │ │
│  │   ⣾  acme/mobile-app          cloning...    45% · 54 MB / 120 MB             │ │
│  │   ⣾  acme/data-pipeline       fetching...   2.1 MB transferred               │ │
│  │   ⣾  acme/terraform-infra     cloning...    12% · 1.8 MB / 15.2 MB           │ │
│  │   ✓  acme/auth-service        already up-to-date                             │ │
│  │   ✗  prakash/deleted-repo     ERROR: 404 Not Found                           │ │
│  │   —  acme/legacy-auth         queued...                                      │ │
│  │   —  acme/docs-site           queued...                                      │ │
│  │   ...                                                                        │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Progress bar mechanics:**
- Top bar uses `Gauge` or custom-rendered Unicode blocks (█░)
- Color transitions: cyan (active phase) → green (complete)
- Live throughput calculated over a sliding 5-second window
- ETA based on rolling average of completed operations

**Live log mechanics:**
- Each line is one repo operation
- Lines update in place (not appended — the entry for `mobile-app` stays on the same row and updates)
- Completed items scroll off the top; active items remain visible
- Errors stay pinned until acknowledged
- `e` filters the log to show only errors
- `f` follows the latest entries (auto-scroll)

**Key Bar (During Sync):**
```
 p:Pause  c:Cancel  e:Show Errors  a:Show All  f:Follow  ↑↓:Scroll  Tab:Next  q:Quit
```

**Pause behavior:**
- `p` pauses new operations. Active clones finish. Queued repos hold.
- Key bar changes: `r:Resume  c:Cancel`
- Paused state indicated by blinking `⏸ PAUSED` in title bar

### 4c. Post-Sync: Results View

```
┌─ Sync Complete ──────────────────────────────────────────────────────────────────┐
│                                                                                  │
│                         ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓                            │
│                         ▓▓  ✓  Sync Complete  ▓▓▓▓                               │
│                         ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓                            │
│                                                                                  │
│   47 repos processed in 00:03:52                                                 │
│                                                                                  │
│   ✓  44 succeeded                                                                │
│      22 new clones      (324 MB)                                                 │
│      22 synced          (+47 commits total)                                      │
│                                                                                  │
│   ✗  3 failed                                                                    │
│      prakash/deleted-repo   404 Not Found                                        │
│      acme/legacy-db         Authentication failed                                │
│      acme/large-repo        Disk full (1.2 GB needed, 87 MB available)           │
│                                                                                  │
│   Total data transferred: 1.8 GB                                                 │
│                                                                                  │
│              [ View Failed Repos ]    [ Return to Dashboard ]                    │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Results interaction:**
- `Enter` on "View Failed Repos" → jumps to Explorer filtered to show only errored repos
- `Enter` on "Return to Dashboard" → jumps to Dashboard tab
- Failed repo list is scrollable if >5 items
- Summary can be exported via `e` → saves to `~/.config/git-vacuum/sync-log.json`

---

## 5. Activity Log (Tab 4)

**Purpose:** Historical record of all sync operations, filterable.

```
┌─ Activity Log ───────────────────────────────────────────────────────────────────┐
│  Filter: /jun                          Show: [All ▾]  Sort: [Newest ▾]          │
│                                                                                  │
│  ┌─ Sync History ───────────────────────────────────────────────────────────────┐ │
│  │                                                                              │ │
│  │   Date           Duration   Repos    Cloned   Updated   Failed   Status       │ │
│  │   ────────────── ────────── ──────── ──────── ──────── ──────── ──────────── │ │
│  │ ▸ 2026-06-16     00:03:52   47       22       22        3        ✓ 44/47     │ │
│  │   14:22 UTC                                                                  │ │
│  │   2026-06-15     00:12:18   128      5        120       3        ✓ 125/128   │ │
│  │   08:15 UTC                                                                  │ │
│  │   2026-06-14     00:01:05   12       12       0         0        ✓ 12/12     │ │
│  │   23:40 UTC                                                                  │ │
│  │   2026-06-14     00:00:42   5        5        0         0        ✓ 5/5       │ │
│  │   18:00 UTC                                                                  │ │
│  │   ...                                                                        │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│  ┌─ Selected Run Detail ────────────────────────────────────────────────────────┐ │
│  │  Run: 2026-06-16 14:22 UTC                                                   │ │
│  │  Trigger: Manual (user initiated from Explorer)                              │ │
│  │  Options: concurrency=8, protocol=SSH, mirror=false                          │ │
│  │                                                                              │ │
│  │  Failed repos in this run:                                                   │ │
│  │    ✗ prakash/deleted-repo    404 Not Found                                   │ │
│  │    ✗ acme/legacy-db          Authentication failed — token may be expired    │ │
│  │    ✗ acme/large-repo         Disk full                                       │ │
│  │                                                                              │ │
│  │  [Retry Failed]  [Export as JSON]                                            │ │
│  └──────────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Layout:** 60/40 vertical split. Top = sync history table. Bottom = detail for selected run.

**Key Bar (Activity Log):**
```
 Enter:View Detail  r:Retry Selected  e:Export  /:Filter  ↑↓:Navigate  Tab:Next
```

**Log storage:** JSON Lines file at `~/.local/share/git-vacuum/activity.jsonl`. Each run is one line. Auto-rotated — keeps last 500 runs, oldest dropped.

---

## 6. Settings Screen (Tab 5)

**Purpose:** Configure authentication, sync preferences, and appearance.

```
┌─ Settings ───────────────────────────────────────────────────────────────────────┐
│                                                                                  │
│  ┌─ Navigation ────────┐  ┌─ Content ───────────────────────────────────────────┐ │
│  │                      │  │                                                     │ │
│  │ ▸ Authentication     │  │  ┌─ GitHub Authentication ─────────────────────────┐ │ │
│  │   Sync Defaults      │  │  │                                                  │ │ │
│  │   Filters            │  │  │  Provider:  [GitHub ▾]                           │ │ │
│  │   Appearance         │  │  │  Token:     ••••••••••••••••  [Change]  [Clear]  │ │ │
│  │   Keybindings        │  │  │  Method:    [▸ Personal Access Token]            │ │ │
│  │   About              │  │  │             [  OAuth Device Flow]                │ │ │
│  │                      │  │  │             [  gh CLI Token (auto-detect)]       │ │ │
│  │                      │  │  │                                                  │ │ │
│  │                      │  │  │  Status: ✓ Authenticated as prakash              │ │ │
│  │                      │  │  │  Scopes: repo, read:org, workflow               │ │ │
│  │                      │  │  │  Expires: 2026-12-15                             │ │ │
│  │                      │  │  │  [Test Connection]                               │ │ │
│  │                      │  │  └──────────────────────────────────────────────────┘ │ │
│  │                      │  │                                                     │ │
│  │                      │  │  ┌─ Git Configuration ─────────────────────────────┐ │ │
│  │                      │  │  │  Protocol:    [SSH ▾]  (or HTTPS)               │ │ │
│  │                      │  │  │  Git binary:  /usr/bin/git  [Auto-detect]       │ │ │
│  │                      │  │  │  SSH key:     ~/.ssh/id_ed25519                 │ │ │
│  │                      │  │  └──────────────────────────────────────────────────┘ │ │
│  └──────────────────────┘  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Layout:** Left sidebar (25%) with settings categories. Right panel (75%) with the selected category's form.

**Settings categories:**

| Category | Content |
|----------|---------|
| **Authentication** | Provider selector, token management, auth method chooser, test connection button |
| **Sync Defaults** | Default clone path, default concurrency, default protocol, auto-prune toggle, include-wikis default, LFS default |
| **Filters** | Default filter presets (always skip forks, always skip archived, default org filter) |
| **Appearance** | Color scheme selector (dark/light/custom), compact mode toggle, icon set toggle (Nerd Fonts vs ASCII), date format |
| **Keybindings** | Read-only table showing all keybindings grouped by context, with ability to remap (v2.0) |
| **About** | Version, license, credits, check for updates button |

**Key Bar (Settings):**
```
 ↑↓:Navigate  Enter:Edit  Tab:Switch Section  Esc:Back  Ctrl+S:Save  q:Quit
```

**Form editing:**
- `Enter` on a field enters edit mode
- Dropdowns: `Enter` opens dropdown, `↑↓` to select, `Enter` to confirm
- Text inputs: type directly, `Enter` to confirm, `Esc` to cancel
- Toggles: `Space` flips
- `Ctrl+S` saves all changes; unsaved changes show `●` indicator in tab bar

---

## 7. Authentication Flow

**Purpose:** First-run experience. Users must authenticate before seeing any other screen.

### 7a. Auth Gate Screen (first launch, or token expired)

```
┌─ Welcome to Git-Vacuum ─────────────────────────────────────────────────────────┐
│                                                                                  │
│                         ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓                         │
│                         ▓▓▓▓▓▓  GIT-VACUUM  ▓▓▓▓▓▓▓                             │
│                         ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓                         │
│                                                                                  │
│   Connect your GitHub account to discover and sync your repositories.            │
│                                                                                  │
│   ┌─ Choose Authentication Method ──────────────────────────────────────────────┐ │
│   │                                                                             │ │
│   │  [▸ Personal Access Token  ]  ── Classic token with repo scope              │ │
│   │  [  OAuth Device Flow      ]  ── Browser-based, no token to copy            │ │
│   │  [  gh CLI Token           ]  ── Uses your existing `gh auth token`         │ │
│   │                                                                             │ │
│   └─────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│   Enter your GitHub Personal Access Token:                                       │
│   ┌──────────────────────────────────────────────────────────────────────────┐   │
│   │  ••••••••••••••••••••••••••••••                                          │   │
│   └──────────────────────────────────────────────────────────────────────────┘   │
│                                                                                  │
│   Token needs:  repo  read:org  workflow  (read-only is sufficient)              │
│                                                                                  │
│   [Connect]                                                  [?] What's a PAT?   │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 7b. OAuth Device Flow (if selected)

```
┌─ Device Activation ─────────────────────────────────────────────────────────────┐
│                                                                                  │
│   1. Open this URL in your browser:                                              │
│                                                                                  │
│      ┌──────────────────────────────────────────────────────────────────────┐    │
│      │  https://github.com/login/device                                      │    │
│      └──────────────────────────────────────────────────────────────────────┘    │
│                                                                                  │
│   2. Enter this code:                                                            │
│                                                                                  │
│      ┌──────────────────────────────────────────────────────────────────────┐    │
│      │                                                                      │    │
│      │               ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓                    │    │
│      │               ▓▓▓▓   XK7F - 2MPQ   ▓▓▓▓▓▓▓                        │    │
│      │               ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓                    │    │
│      │                                                                      │    │
│      └──────────────────────────────────────────────────────────────────────┘    │
│                                                                                  │
│   Status:  ⣾ Waiting for authorization... (timeout in 14:52)                     │
│                                                                                  │
│   [Cancel]  [Copy Code to Clipboard]                                             │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 7c. Validating State

```
┌─ Authenticating... ─────────────────────────────────────────────────────────────┐
│                                                                                  │
│                         ⣾  Verifying credentials with GitHub...                   │
│                                                                                  │
│   Checking token scopes...                                                       │
│   Fetching user profile...                                                       │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 7d. Auth Success → Transition

On successful auth:
1. Token is stored in OS keychain (via `keyring` crate)
2. Short flash of `✓ Connected as @prakash` 
3. Auto-transition to Dashboard (or Explorer if `--explorer` flag was passed)

### 7e. Auth Failure States

```
┌─ Authentication Failed ─────────────────────────────────────────────────────────┐
│                                                                                  │
│                         ✗  Invalid credentials                                   │
│                                                                                  │
│   The token you provided was rejected by GitHub.                                 │
│                                                                                  │
│   Common causes:                                                                 │
│   • Token has expired (check GitHub → Settings → Developer settings → Tokens)    │
│   • Token lacks required scopes (needs: repo, read:org)                          │
│   • Token was revoked                                                             │
│   • Network connectivity issue (could not reach api.github.com)                  │
│                                                                                  │
│   [Try Again]  [Use Different Method]  [Skip Auth (public repos only)]           │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Modal System

**Design principle:** Modals overlay the main content with a dimmed backdrop. They have clear entry/exit points. Never more than one modal at a time.

### Modal Anatomy

```
┌─ Main Content (dimmed, unclickable) ─────────────────────────────────────────────┐
│                                                                                  │
│    ┌─ Backdrop (dimmed area) ─────────────────────────────────────────────┐     │
│    │                                                                       │     │
│    │   ┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐     │
│    │   ╎  Modal Title                                          [X]  ╎     │
│    │   ╎────────────────────────────────────────────────────────────╎     │
│    │   ╎                                                             ╎     │
│    │   ╎                    Modal Content                            ╎     │
│    │   ╎                                                             ╎     │
│    │   ╎────────────────────────────────────────────────────────────╎     │
│    │   ╎  [Cancel]                              [Confirm Action]    ╎     │
│    │   └╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘     │
│    │                                                                       │     │
│    └───────────────────────────────────────────────────────────────────────┘     │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Modal Types

#### Confirmation Modal
Used before destructive or irreversible actions.

```
┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐
╎  Prune Local Repositories                              [Esc] ╎
╎──────────────────────────────────────────────────────────────╎
╎                                                               ╎
╎  This will permanently delete 3 local repositories            ╎
╎  that no longer exist on GitHub:                              ╎
╎                                                               ╎
╎    • acme/archived-project      (last synced: 2026-01-15)     ╎
╎    • acme/old-docs              (last synced: 2025-11-03)     ╎
╎    • prakash/test-repo          (last synced: 2026-03-22)     ╎
╎                                                               ╎
╎  ⚠ This cannot be undone.                                    ╎
╎                                                               ╎
╎──────────────────────────────────────────────────────────────╎
╎  [No, keep them]                [Yes, delete permanently]    ╎
└╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘
```

Default focus is on the safe option (`No, keep them`). Destructive option requires `Tab` to reach.

#### Repo Detail Modal
Drilled-in view for inspecting a single repository.

```
┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐
╎  acme/web-frontend                                                         [Esc] ╎
╎─────────────────────────────────────────────────────────────────────────────────╎
╎                                                                                  ╎
╎  ┌─ Remote ──────────────────────────┐  ┌─ Local ───────────────────────────────┐╎
╎  │  Language:    TypeScript          │  │  Status:    ✓ Cloned                   │╎
╎  │  Stars:       247                 │  │  Path:      ~/git-vacuum/acme/web-…   │╎
╎  │  Default:     main                │  │  Size:      23.7 MB (working tree)     │╎
╎  │  License:     MIT                 │  │  Last sync: 2026-06-14 08:15 UTC       │╎
╎  │  Visibility:  public              │  │  Behind by: 14 commits                  │╎
╎  │  Last push:   2026-06-15          │  │  Local branch: main                    │╎
╎  │  Archived:    No                  │  │                                          │╎
╎  │  Fork:        No                  │  │                                          │╎
╎  └───────────────────────────────────┘  └──────────────────────────────────────────┘╎
╎                                                                                  ╎
╎  ┌─ Recent Commits (remote) ───────────────────────────────────────────────────┐ ╎
╎  │  a7f3c2b  Fix: resolve login redirect loop     · Jane Doe   · 2026-06-15    │ ╎
╎  │  b2d8e1a  Feat: add dark mode toggle            · Jane Doe   · 2026-06-14    │ ╎
╎  │  c9e4f0d  Chore: update dependencies             · CI Bot     · 2026-06-13    │ ╎
╎  └──────────────────────────────────────────────────────────────────────────────┘ ╎
╎                                                                                  ╎
╎──────────────────────────────────────────────────────────────────────────────────╎
╎  [Sync Now]  [Open in Editor]  [Open in Browser]  [Copy Clone URL]  [Forget]    ╎
└╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘
```

#### Error Detail Modal
When user clicks into an error from the log.

```
┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐
╎  Error Detail                                           [Esc] ╎
╎──────────────────────────────────────────────────────────────╎
╎                                                               ╎
╎  Repository:  acme/legacy-db                                  ╎
╎  Operation:   Clone                                           ╎
╎  Time:        2026-06-16 14:23:15 UTC                         ╎
╎                                                               ╎
╎  ┌─ Error Message ───────────────────────────────────────────┐╎
╎  │  Authentication failed. The provided token does not       │╎
╎  │  have permission to access this repository.               │╎
╎  │                                                            │╎
╎  │  This repository may be private and your token may lack   │╎
╎  │  the required 'repo' scope, or your account may not       │╎
╎  │  have access to this organization.                        │╎
╎  └────────────────────────────────────────────────────────────┘╎
╎                                                               ╎
╎  ┌─ Raw Output ──────────────────────────────────────────────┐╎
╎  │  remote: Repository not found.                            │╎
╎  │  fatal: repository 'https://github.com/acme/legacy-db'   │╎
╎  │  not found                                                │╎
╎  └────────────────────────────────────────────────────────────┘╎
╎                                                               ╎
╎──────────────────────────────────────────────────────────────╎
╎  [Copy Error]  [Retry]  [Dismiss]                            ╎
└╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘
```

#### Help Overlay
Triggered by `?` from any screen. Full keyboard reference.

```
┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐
╎  Keyboard Shortcuts                                                            [Esc] ╎
╎──────────────────────────────────────────────────────────────────────────────────────╎
╎                                                                                       ╎
╎  ┌─ Global ────────────────┐  ┌─ Explorer ──────────────┐  ┌─ Sync Center ───────────┐╎
╎  │  1-5   Switch tab       │  │  Space  Toggle selection │  │  Enter   Start sync     │╎
╎  │  Tab   Next tab         │  │  v      Mark mode        │  │  p       Pause          │╎
╎  │  Shift+Tab  Prev tab    │  │  Ctrl+A Select all       │  │  r       Resume         │╎
╎  │  q      Quit            │  │  Ctrl+D Deselect all     │  │  c       Cancel         │╎
╎  │  ?      This help       │  │  /      Filter repos     │  │  e       Show errors    │╎
╎  │  :      Command palette │  │  1-6    Sort by column   │  │  f       Follow log     │╎
╎  │  Esc    Back / Close    │  │  Enter  Inspect repo     │  │  ↑↓      Scroll log     │╎
╎  │  ↑↓     Navigate        │  │  s      Sync selected    │  │                         │╎
╎  └─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘╎
╎                                                                                       ╎
╎  ┌─ Dashboard ─────────────┐  ┌─ Activity Log ──────────┐  ┌─ Settings ──────────────┐╎
╎  │  Enter  Inspect repo    │  │  Enter  View run detail │  │  Enter   Edit field     │╎
╎  │  s      Start sync      │  │  r      Retry failed    │  │  Ctrl+S  Save changes    │╎
╎  │  r      Refresh stats   │  │  e      Export as JSON  │  │  Esc     Discard changes │╎
╎  └─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘╎
│                                                                                       │
│  Press any key to close                                                                │
└╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘
```

**Modal system rules:**
- Dim backdrop uses a 50% opacity overlay character or reverses background colors
- `Esc` always dismisses the topmost modal
- Modal content is never taller than 80% of terminal height — scrolls internally if needed
- Focus trapping: Tab cycles between buttons/inputs within the modal only
- Backdrop click (if mouse enabled) dismisses non-critical modals

---

## 9. Command Palette

**Purpose:** Power-user quick access. Triggered by `:` from any screen (like k9s, vim).

```
┌─ Main Content (dimmed) ──────────────────────────────────────────────────────────┐
│                                                                                  │
│  ┌─ Command Palette ─────────────────────────────────────────────────────────┐   │
│  │  :sync                                                                     │   │
│  │────────────────────────────────────────────────────────────────────────────│   │
│  │  ▸ sync                     Start sync for selected repos                  │   │
│  │    sync --mirror            Start mirror-mode sync                         │   │
│  │    sync --prune             Sync and prune deleted repos                   │   │
│  │    sync all                 Sync all repositories                          │   │
│  │    sync failed              Retry all previously failed repos              │   │
│  └────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Command Reference

| Command | Action | Context |
|---------|--------|---------|
| `:sync` | Start sync for selected repos | Explorer, Dashboard |
| `:sync all` | Sync all known repos | Any |
| `:sync failed` | Retry only failed repos from last run | Any |
| `:sync --mirror` | Mirror-mode sync | Explorer |
| `:sync --prune` | Sync + prune deleted repos | Explorer |
| `:clone <owner>/<repo>` | Clone a specific repo | Any |
| `:discover` | Re-discover repos from GitHub | Explorer |
| `:discover <org>` | Discover repos for a specific org | Any |
| `:filter <pattern>` | Set filter and jump to Explorer | Any |
| `:export [json\|csv]` | Export repo list to file | Explorer, Dashboard |
| `:prune` | Prune local repos with no remote | Dashboard |
| `:auth` | Open auth settings | Any |
| `:auth switch` | Switch GitHub account | Settings |
| `:config` | Jump to settings → specific section | Any |
| `:help` | Open help overlay | Any |
| `:quit` / `:q` | Exit Git-Vacuum | Any |
| `:version` | Show version info | Any |

**Palette behavior:**
- `:` opens palette with empty prompt
- Typing live-filters the command list (fuzzy match)
- `Enter` executes the top match; `Esc` dismisses
- `Tab` auto-completes the current word
- Command history: `↑`/`↓` cycles through previous commands (stored per-session)
- Invalid commands show `✗ Unknown command: :xyz` in the palette

---

## 10. Keyboard Shortcuts — Complete Reference

### Global (available on all screens)

| Key | Action |
|-----|--------|
| `1`–`5` | Switch to tab 1–5 |
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `q` | Quit Git-Vacuum |
| `?` | Open help overlay |
| `:` | Open command palette |
| `Esc` | Dismiss modal / go back / clear filter |
| `↑` / `↓` | Navigate up/down in lists |
| `PgUp` / `PgDn` | Page up/down in scrollable lists |
| `Home` / `End` | Jump to top/bottom of list |
| `Ctrl+C` | Force quit (emergency) |

### Dashboard (Tab 1)

| Key | Action |
|-----|--------|
| `Enter` | Inspect selected repo (modal) |
| `s` | Start sync for all stale repos |
| `r` | Refresh stats from local git data |
| `↑` / `↓` | Navigate attention list |

### Explorer (Tab 2)

| Key | Action |
|-----|--------|
| `Space` | Toggle selection on current repo |
| `v` | Enter visual mark mode |
| `Shift+↓` / `Shift+↑` | Range-select in mark mode |
| `Ctrl+A` | Select all visible repos |
| `Ctrl+D` | Deselect all |
| `Enter` | Clone/sync selected repos (or inspect if single) |
| `s` | Sync selected repos |
| `/` | Focus filter input |
| `1`–`6` | Sort table by column 1–6 |
| `r` | Re-discover repos from GitHub API |
| `o` | Open selected repo in browser |
| `c` | Copy clone URL to clipboard |

### Sync Center (Tab 3)

| Key | Action |
|-----|--------|
| `Enter` | Start sync (pre-sync) / View failed details (post-sync) |
| `p` | Pause active sync |
| `r` | Resume paused sync |
| `c` | Cancel sync (with confirmation) |
| `e` | Filter log to show only errors |
| `a` | Show all log entries |
| `f` | Follow latest log entries (auto-scroll) |
| `↑` / `↓` | Scroll log |

### Activity Log (Tab 4)

| Key | Action |
|-----|--------|
| `Enter` | View detail of selected run |
| `r` | Retry failed repos from selected run |
| `e` | Export selected run as JSON |
| `/` | Filter runs by date/repo name |
| `Shift+F` | Toggle filter: All / Failed only / With errors |

### Settings (Tab 5)

| Key | Action |
|-----|--------|
| `Enter` | Edit selected field / Drill into section |
| `Space` | Toggle boolean fields |
| `Ctrl+S` | Save all changed settings |
| `Esc` | Discard current field edit / Go back to section list |
| `↑` / `↓` | Navigate settings fields |
| `Tab` | Switch between nav sidebar and content |

### Modal Shortcuts

| Key | Action |
|-----|--------|
| `Esc` | Dismiss modal |
| `Tab` | Next button/field in modal |
| `Shift+Tab` | Previous button/field |
| `Enter` | Activate focused button |
| `y` / `n` | Quick yes/no in confirmation dialogs |

---

## Design Notes

### Color System
Borrowing from k9s' skin approach but keeping it simple for MVP:

- **Default palette** works on both light and dark terminals (use `terminal::Color::Reset` for transparency where possible)
- **Semantic colors:** green (success), yellow (warning/stale), red (error), cyan (active/in-progress), blue (selection), gray (dimmed/disabled)
- **Theme file support** deferred to v1.0 (single built-in theme for MVP)

### Icon Strategy
- **Nerd Font icons** enabled by default (nerd font glyphs for status: `nf-md-check`, `nf-md-arrow_up`, `nf-md-alert`, etc.)
- **ASCII fallback** via config toggle `noIcons: true` (replaces nerd font chars with `[OK]`, `[UP]`, `[ERR]`)
- Auto-detect terminal font support on first launch

### Responsive Behavior
- **Minimum terminal size:** 80×24 characters
- Below minimum: show warning "Terminal too small. Resize to at least 80×24."
- **Table columns** collapse in priority order when width shrinks: Size → Visibility → Status → Owner. Name is always visible.
- **Panels** resize proportionally. Detail panel in Explorer can collapse to 0% width on very narrow terminals.

### Loading States
- **API calls:** Spinner (`⣾⣽⣻⢿⡿⣟⣯⣷`) in the affected area, not a global freeze
- **Initial data load:** Dashboard shows skeleton/placeholder rectangles that fill in as data arrives
- **Sync operations:** Per-repo spinners shown alongside live progress data

### Accessibility
- All status information is conveyed through both color AND text/icons (never color alone)
- Keyboard navigation covers 100% of functionality
- Mouse is optional (`enableMouse: false` by default, matching k9s convention)
