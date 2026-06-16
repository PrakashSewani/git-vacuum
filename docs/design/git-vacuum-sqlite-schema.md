# Git-Vacuum — SQLite Schema Design

**Database:** SQLite 3 (via `rusqlite`, embedded, zero-configuration)  
**File location:** `~/.local/share/git-vacuum/git-vacuum.db` (XDG data dir)  
**Collation:** Default (binary). Case-insensitive search handled via `COLLATE NOCASE` on `full_name`.  
**Date/time:** ISO 8601 text strings (`2026-06-16T14:22:00Z`). SQLite has no native datetime type; text is sortable, portable, and human-readable.  
**Booleans:** `INTEGER NOT NULL DEFAULT 0` with `CHECK (col IN (0, 1))`. SQLite has no native boolean.

---

## 1. Entity-Relationship Diagram

```
┌──────────────────────┐       ┌──────────────────────┐
│   github_accounts    │       │     github_orgs       │
│──────────────────────│       │──────────────────────│
│ PK id                │       │ PK id                 │
│    login             │       │    login              │
│    display_name      │       │    display_name       │
│    avatar_url        │       │    avatar_url         │
│    token_scopes      │       │    description        │
│    token_expires_at  │       │    discovered_at      │
│    created_at        │       │    updated_at         │
│    updated_at        │       └──────────┬───────────┘
└──────────┬───────────┘                  │
           │                              │
           │ 1                         M  │
           │                              │
           │    ┌──────────────────────┐  │
           │    │   org_memberships    │  │
           │    │──────────────────────│  │
           │    │ PK account_id  (FK)  │──┤
           │    │ PK org_id      (FK)  │──┘
           │    │    role              │
           │    │    joined_at         │
           │    └──────────────────────┘
           │
           │ 1..M  (owns or contributes to)
           │
           ▼
┌──────────────────────┐       ┌──────────────────────┐
│    repositories      │       │       topics         │
│──────────────────────│       │──────────────────────│
│ PK id                │       │ PK id                │
│    github_id (UQ)    │       │    name (UQ)         │
│ FK owner_org_id      │──┐    │    created_at        │
│ FK owner_account_id  │──┤    └──────────┬───────────┘
│    name               │  │               │
│    full_name (UQ)     │  │               │
│    description        │  │    ┌──────────────────────┐
│    language           │  │    │    repo_topics       │
│    default_branch     │  │    │──────────────────────│
│    visibility         │  │    │ PK repo_id    (FK)   │
│    is_fork            │  │    │ PK topic_id   (FK)   │
│    is_archived        │  │    └──────────────────────┘
│    size_kb            │  │
│    stars              │  │
│    clone_url_ssh      │  │
│    clone_url_https    │  │
│    pushed_at          │  │
│    created_at_gh      │  │
│    updated_at_gh      │  │
│    discovered_at      │  │
│    selected (UI pref) │  │
│    deleted_on_remote  │  │
└──────────┬───────────┘  │
           │               │
           │ 1             │
           │               │
           ▼               │
┌──────────────────────┐  │
│    local_clones       │  │
│──────────────────────│  │
│ PK id                │  │
│ FK repo_id (UQ)      │──┘
│    local_path        │
│    clone_status      │
│    local_size_kb     │
│    behind_count      │
│    ahead_count       │
│    last_synced_at    │
│    last_error        │
│    last_error_at     │
│    created_at        │
│    updated_at        │
└──────────┬───────────┘
           │
           │ 1..M  (participates in many syncs)
           │
           ▼
┌──────────────────────┐       ┌──────────────────────┐
│    sync_runs          │       │    sync_entries       │
│──────────────────────│       │──────────────────────│
│ PK id                │──┐    │ PK id                │
│    started_at        │  │    │ FK run_id            │──┘
│    completed_at      │  │    │ FK repo_id           │──▶ repositories
│    status            │  │    │    operation          │
│    trigger           │  │    │    entry_status       │
│    total_repos       │  │    │    bytes_transferred  │
│    cloned_count      │  │    │    new_commits        │
│    updated_count     │  │    │    duration_ms        │
│    skipped_count     │  │    │    error_message      │
│    failed_count      │  │    │    started_at         │
│    bytes_transferred │  │    │    completed_at       │
│    options_json      │  │    └──────────────────────┘
└──────────┬───────────┘  │
           │               │
           │ 1..M          │
           │               │
           ▼               │
┌──────────────────────┐  │
│     sync_log          │  │
│──────────────────────│  │
│ PK id                │  │
│ FK run_id            │──┘
│ FK repo_id (nullable)│
│    level             │
│    message           │
│    timestamp         │
└──────────────────────┘

┌──────────────────────┐
│      settings         │
│──────────────────────│
│ PK key               │
│    value             │
│    updated_at        │
└──────────────────────┘
```

**Key relationships:**
- `github_accounts` 1──M `repositories` (repos owned by a user account)
- `github_orgs` 1──M `repositories` (repos owned by an org)
- `github_accounts` M──M `github_orgs` via `org_memberships`
- `repositories` 1──1 `local_clones` (each repo has at most one local clone)
- `sync_runs` 1──M `sync_entries` (each run has many per-repo results)
- `sync_runs` 1──M `sync_log` (each run has many log lines)
- `repositories` M──M `topics` via `repo_topics`

**Design decision — separate `repositories` from `local_clones`:**
A repository can exist on GitHub without being cloned locally. Its remote metadata (description, stars, pushed_at) can change independently of local state (clone_status, behind_count). Separating them means we can refresh remote metadata via the API without touching or invalidating local state. The common query ("show all repos with their clone status") uses a LEFT JOIN. For performance, we add a denormalized view `repo_dashboard` that pre-joins these tables — the service layer reads the view for the Explorer and Dashboard screens.

---

## 2. Table Definitions

### 2.1 `schema_version`

Tracks applied migrations. The application reads this on startup and applies any pending migrations in order.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `version` | INTEGER | PRIMARY KEY | Migration version number |
| `name` | TEXT | NOT NULL | Human-readable migration name |
| `applied_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | ISO 8601 timestamp |

```sql
CREATE TABLE schema_version (
    version   INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 2.2 `github_accounts`

Authenticated GitHub user accounts. MVP has exactly one row. Multi-account support (future) allows multiple rows — the active account is tracked via `settings`.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Surrogate key |
| `github_user_id` | INTEGER | NOT NULL UNIQUE | GitHub's internal user ID |
| `login` | TEXT | NOT NULL | GitHub username (e.g., "prakash") |
| `display_name` | TEXT | | Full name from GitHub profile |
| `email` | TEXT | | Primary email from GitHub |
| `avatar_url` | TEXT | | URL to avatar image |
| `token_scopes` | TEXT | | Comma-separated OAuth scopes (e.g., "repo,read:org") |
| `token_expires_at` | TEXT | | ISO 8601. NULL if token never expires (classic PAT) |
| `token_stored_in_keyring` | INTEGER | NOT NULL DEFAULT 1 CHECK(token_stored_in_keyring IN (0,1)) | Whether token is in OS keyring |
| `last_validated_at` | TEXT | | Last successful token validation |
| `created_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |
| `updated_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |

```sql
CREATE TABLE github_accounts (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    github_user_id          INTEGER NOT NULL UNIQUE,
    login                   TEXT NOT NULL,
    display_name            TEXT,
    email                   TEXT,
    avatar_url              TEXT,
    token_scopes            TEXT,
    token_expires_at        TEXT,
    token_stored_in_keyring INTEGER NOT NULL DEFAULT 1
                                CHECK(token_stored_in_keyring IN (0, 1)),
    last_validated_at       TEXT,
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_accounts_login ON github_accounts(login);
```

**Note:** The actual OAuth/PAT token is never stored in SQLite. It lives exclusively in the OS keyring (via the `keyring` crate). The `token_scopes` and `token_expires_at` columns cache metadata for the Settings screen without needing to decode the token.

### 2.3 `github_orgs`

Organizations the authenticated user has access to. Populated during discovery.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `github_org_id` | INTEGER | NOT NULL UNIQUE | GitHub's internal org ID |
| `login` | TEXT | NOT NULL | Org slug (e.g., "acme-corp") |
| `display_name` | TEXT | | Human-readable org name |
| `description` | TEXT | | Org description from GitHub |
| `avatar_url` | TEXT | | |
| `repos_count` | INTEGER | NOT NULL DEFAULT 0 | Denormalized: count of repos in this org, updated during discovery |
| `discovered_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |
| `updated_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |

```sql
CREATE TABLE github_orgs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    github_org_id   INTEGER NOT NULL UNIQUE,
    login           TEXT NOT NULL,
    display_name    TEXT,
    description     TEXT,
    avatar_url      TEXT,
    repos_count     INTEGER NOT NULL DEFAULT 0,
    discovered_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_orgs_login ON github_orgs(login);
```

### 2.4 `org_memberships`

Junction table tracking which accounts belong to which orgs.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `account_id` | INTEGER | NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE | |
| `org_id` | INTEGER | NOT NULL REFERENCES github_orgs(id) ON DELETE CASCADE | |
| `role` | TEXT | NOT NULL DEFAULT 'member' CHECK(role IN ('admin','member')) | User's role in the org |
| `joined_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |

```sql
CREATE TABLE org_memberships (
    account_id  INTEGER NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
    org_id      INTEGER NOT NULL REFERENCES github_orgs(id) ON DELETE CASCADE,
    role        TEXT NOT NULL DEFAULT 'member' CHECK(role IN ('admin', 'member')),
    joined_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (account_id, org_id)
);
```

### 2.5 `repositories`

Cached GitHub repository metadata. The **canonical source of remote truth** between API calls. Updated atomically during each discovery run.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `github_id` | INTEGER | NOT NULL UNIQUE | GitHub's internal repo ID |
| `owner_org_id` | INTEGER | REFERENCES github_orgs(id) ON DELETE SET NULL | NULL if owned by a user account |
| `owner_account_id` | INTEGER | REFERENCES github_accounts(id) ON DELETE SET NULL | NULL if owned by an org |
| `owner_login` | TEXT | NOT NULL | Denormalized: owner's login (redundant with FK but enables fast queries without join) |
| `name` | TEXT | NOT NULL | Repo name without owner (e.g., "web-frontend") |
| `full_name` | TEXT | NOT NULL UNIQUE | "owner/name" (e.g., "acme/web-frontend") |
| `description` | TEXT | | |
| `language` | TEXT | | Primary language detected by GitHub |
| `default_branch` | TEXT | NOT NULL DEFAULT 'main' | |
| `visibility` | TEXT | NOT NULL CHECK(visibility IN ('public','private','internal')) | |
| `is_fork` | INTEGER | NOT NULL DEFAULT 0 CHECK(is_fork IN (0,1)) | |
| `is_archived` | INTEGER | NOT NULL DEFAULT 0 CHECK(is_archived IN (0,1)) | |
| `is_template` | INTEGER | NOT NULL DEFAULT 0 CHECK(is_template IN (0,1)) | |
| `size_kb` | INTEGER | | Size in KB as reported by GitHub API |
| `stars` | INTEGER | NOT NULL DEFAULT 0 | |
| `open_issues` | INTEGER | NOT NULL DEFAULT 0 | |
| `license_spdx` | TEXT | | SPDX identifier (e.g., "MIT") |
| `topics_json` | TEXT | | JSON array of topic strings (denormalized for fast reads without join) |
| `clone_url_ssh` | TEXT | | |
| `clone_url_https` | TEXT | | |
| `homepage_url` | TEXT | | |
| `pushed_at` | TEXT | | ISO 8601. Last push to any branch. |
| `created_at_gh` | TEXT | | When the repo was created on GitHub |
| `updated_at_gh` | TEXT | | Last update event on GitHub |
| `selected` | INTEGER | NOT NULL DEFAULT 1 CHECK(selected IN (0,1)) | UI preference: checked by default in Explorer |
| `deleted_on_remote` | INTEGER | NOT NULL DEFAULT 0 CHECK(deleted_on_remote IN (0,1)) | Set to 1 when repo no longer exists on GitHub (prune candidate) |
| `discovered_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | Last time this repo was seen in a discovery run |
| `created_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | Local row creation |
| `updated_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | Local row update |

```sql
CREATE TABLE repositories (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    github_id         INTEGER NOT NULL UNIQUE,
    owner_org_id      INTEGER REFERENCES github_orgs(id) ON DELETE SET NULL,
    owner_account_id  INTEGER REFERENCES github_accounts(id) ON DELETE SET NULL,
    owner_login       TEXT NOT NULL,
    name              TEXT NOT NULL,
    full_name         TEXT NOT NULL UNIQUE,
    description       TEXT,
    language          TEXT,
    default_branch    TEXT NOT NULL DEFAULT 'main',
    visibility        TEXT NOT NULL CHECK(visibility IN ('public', 'private', 'internal')),
    is_fork           INTEGER NOT NULL DEFAULT 0 CHECK(is_fork IN (0, 1)),
    is_archived       INTEGER NOT NULL DEFAULT 0 CHECK(is_archived IN (0, 1)),
    is_template       INTEGER NOT NULL DEFAULT 0 CHECK(is_template IN (0, 1)),
    size_kb           INTEGER,
    stars             INTEGER NOT NULL DEFAULT 0,
    open_issues       INTEGER NOT NULL DEFAULT 0,
    license_spdx      TEXT,
    topics_json       TEXT,
    clone_url_ssh     TEXT,
    clone_url_https   TEXT,
    homepage_url      TEXT,
    pushed_at         TEXT,
    created_at_gh     TEXT,
    updated_at_gh     TEXT,
    selected          INTEGER NOT NULL DEFAULT 1 CHECK(selected IN (0, 1)),
    deleted_on_remote INTEGER NOT NULL DEFAULT 0 CHECK(deleted_on_remote IN (0, 1)),
    discovered_at     TEXT NOT NULL DEFAULT (datetime('now')),
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Lookups
CREATE INDEX idx_repos_full_name ON repositories(full_name COLLATE NOCASE);
CREATE INDEX idx_repos_owner ON repositories(owner_login);
CREATE INDEX idx_repos_github_id ON repositories(github_id);

-- Explorer filtering
CREATE INDEX idx_repos_visibility ON repositories(visibility);
CREATE INDEX idx_repos_fork ON repositories(is_fork) WHERE is_fork = 1;
CREATE INDEX idx_repos_archived ON repositories(is_archived) WHERE is_archived = 1;

-- Discovery: find repos not seen in a while (prune candidates)
CREATE INDEX idx_repos_discovered ON repositories(discovered_at);

-- Sorting in Explorer
CREATE INDEX idx_repos_pushed ON repositories(pushed_at);
CREATE INDEX idx_repos_stars ON repositories(stars);
```

**Design decision — `topics_json` denormalization:**
GitHub topics are an array of short strings (e.g., `["rust", "cli", "devops"]`). For Explorer's "filter by topic" feature, joining through a normalized `repo_topics` + `topics` table is correct but adds latency for the most common query. Instead, we store topics as a JSON array in `repositories.topics_json` for instant reads. The normalized `topics` and `repo_topics` tables exist for future use (analytics, topic cloud) but are not queried during normal Explorer rendering. This is a controlled denormalization — the JSON column is always regenerated from the API, never partially updated.

### 2.6 `topics`

Normalized topic reference table (future use).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `name` | TEXT | NOT NULL UNIQUE | Lowercase topic slug |

```sql
CREATE TABLE topics (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);
```

### 2.7 `repo_topics`

Junction table for normalized topic relationship (future use for analytics/topic cloud).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `repo_id` | INTEGER | NOT NULL REFERENCES repositories(id) ON DELETE CASCADE | |
| `topic_id` | INTEGER | NOT NULL REFERENCES topics(id) ON DELETE CASCADE | |

```sql
CREATE TABLE repo_topics (
    repo_id  INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
    PRIMARY KEY (repo_id, topic_id)
);
```

### 2.8 `local_clones`

Tracks the local filesystem state of each cloned repository. **One row per cloned repo.** Rows are INSERTed on first clone, UPDATEd on sync, and DELETEd on prune.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `repo_id` | INTEGER | NOT NULL UNIQUE REFERENCES repositories(id) ON DELETE CASCADE | |
| `local_path` | TEXT | NOT NULL | Absolute path on disk (e.g., `/home/user/git-vacuum/acme/web-frontend`) |
| `clone_status` | TEXT | NOT NULL DEFAULT 'cloned' CHECK(clone_status IN ('cloning','cloned','stale','error')) | |
| `local_size_kb` | INTEGER | | Disk usage of the working tree (not .git), measured periodically |
| `git_dir_size_kb` | INTEGER | | Size of .git directory (object database) |
| `behind_count` | INTEGER | NOT NULL DEFAULT 0 | Commits behind remote (from `git status -sb`) |
| `ahead_count` | INTEGER | NOT NULL DEFAULT 0 | Unpushed local commits |
| `is_dirty` | INTEGER | NOT NULL DEFAULT 0 CHECK(is_dirty IN (0,1)) | Working tree has uncommitted changes |
| `current_branch` | TEXT | | Currently checked-out branch |
| `clone_protocol` | TEXT | NOT NULL CHECK(clone_protocol IN ('ssh','https')) | Protocol used for clone |
| `last_synced_at` | TEXT | | Timestamp of last successful fetch |
| `last_error` | TEXT | | Last error message (NULL if no error) |
| `last_error_at` | TEXT | | Timestamp of last error |
| `consecutive_errors` | INTEGER | NOT NULL DEFAULT 0 | Consecutive sync failures (reset on success) |
| `created_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | When first cloned |
| `updated_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |

```sql
CREATE TABLE local_clones (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id             INTEGER NOT NULL UNIQUE REFERENCES repositories(id) ON DELETE CASCADE,
    local_path          TEXT NOT NULL,
    clone_status        TEXT NOT NULL DEFAULT 'cloned'
                            CHECK(clone_status IN ('cloning', 'cloned', 'stale', 'error')),
    local_size_kb       INTEGER,
    git_dir_size_kb     INTEGER,
    behind_count        INTEGER NOT NULL DEFAULT 0,
    ahead_count         INTEGER NOT NULL DEFAULT 0,
    is_dirty            INTEGER NOT NULL DEFAULT 0 CHECK(is_dirty IN (0, 1)),
    current_branch      TEXT,
    clone_protocol      TEXT NOT NULL CHECK(clone_protocol IN ('ssh', 'https')),
    last_synced_at      TEXT,
    last_error          TEXT,
    last_error_at       TEXT,
    consecutive_errors  INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_clones_status ON local_clones(clone_status);
CREATE INDEX idx_clones_behind ON local_clones(behind_count) WHERE behind_count > 0;
CREATE INDEX idx_clones_error ON local_clones(clone_status) WHERE clone_status = 'error';
```

**`clone_status` lifecycle:**
```
not present ──▶ (no row in local_clones)
                   │
                   │ first clone starts
                   ▼
              'cloning' ────────────────▶ 'cloned'
                   │                          │
                   │ clone fails              │ sync fails
                   ▼                          ▼
              'error' ◀───────────────── 'error'
                   │                          │
                   │ retry succeeds           │ retry succeeds
                   └──────▶ 'cloned' ◀────────┘
                                │
                                │ remote has new commits (detected by stats refresh)
                                ▼
                            'stale'
                                │
                                │ sync succeeds
                                ▼
                            'cloned'
```

### 2.9 `sync_runs`

One row per sync operation. Created when a sync begins, updated when it completes.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `started_at` | TEXT | NOT NULL | ISO 8601 |
| `completed_at` | TEXT | | NULL while running |
| `status` | TEXT | NOT NULL DEFAULT 'running' CHECK(status IN ('running','completed','cancelled','failed')) | Terminal states: completed, cancelled, failed |
| `trigger` | TEXT | NOT NULL DEFAULT 'manual' CHECK(trigger IN ('manual','scheduled','cli')) | What initiated the sync |
| `total_repos` | INTEGER | NOT NULL DEFAULT 0 | Total repos in the sync plan |
| `cloned_count` | INTEGER | NOT NULL DEFAULT 0 | New clones performed |
| `updated_count` | INTEGER | NOT NULL DEFAULT 0 | Repos that were fetched with new commits |
| `skipped_count` | INTEGER | NOT NULL DEFAULT 0 | Repos already up-to-date (no-op) |
| `failed_count` | INTEGER | NOT NULL DEFAULT 0 | Repos that errored |
| `bytes_transferred` | INTEGER | NOT NULL DEFAULT 0 | Total data transferred in bytes |
| `duration_ms` | INTEGER | | Wall-clock duration (set on completion) |
| `options_json` | TEXT | | JSON blob of sync options: {concurrency, protocol, mirror, prune, include_wikis, lfs} |

```sql
CREATE TABLE sync_runs (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at          TEXT NOT NULL,
    completed_at        TEXT,
    status              TEXT NOT NULL DEFAULT 'running'
                            CHECK(status IN ('running', 'completed', 'cancelled', 'failed')),
    trigger             TEXT NOT NULL DEFAULT 'manual'
                            CHECK(trigger IN ('manual', 'scheduled', 'cli')),
    total_repos         INTEGER NOT NULL DEFAULT 0,
    cloned_count        INTEGER NOT NULL DEFAULT 0,
    updated_count       INTEGER NOT NULL DEFAULT 0,
    skipped_count       INTEGER NOT NULL DEFAULT 0,
    failed_count        INTEGER NOT NULL DEFAULT 0,
    bytes_transferred   INTEGER NOT NULL DEFAULT 0,
    duration_ms         INTEGER,
    options_json        TEXT
);

-- Activity Log: show recent runs, newest first
CREATE INDEX idx_sync_runs_started ON sync_runs(started_at DESC);

-- Filter activity log by status
CREATE INDEX idx_sync_runs_status ON sync_runs(status);
```

### 2.10 `sync_entries`

Per-repository results within a sync run. One row per repo per run.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `run_id` | INTEGER | NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE | |
| `repo_id` | INTEGER | NOT NULL REFERENCES repositories(id) ON DELETE CASCADE | |
| `operation` | TEXT | NOT NULL CHECK(operation IN ('clone','sync','skip')) | What the engine decided to do |
| `entry_status` | TEXT | NOT NULL CHECK(entry_status IN ('running','success','failed')) | |
| `bytes_transferred` | INTEGER | NOT NULL DEFAULT 0 | |
| `new_commits` | INTEGER | NOT NULL DEFAULT 0 | New commits pulled (0 for clones, ≥0 for syncs) |
| `duration_ms` | INTEGER | | Per-repo operation time |
| `error_code` | TEXT | | Machine-readable error code (e.g., "AUTH_FAILED", "DISK_FULL") |
| `error_message` | TEXT | | Human-readable error |
| `started_at` | TEXT | | Per-repo start time |
| `completed_at` | TEXT | | Per-repo end time |

```sql
CREATE TABLE sync_entries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    repo_id             INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    operation           TEXT NOT NULL CHECK(operation IN ('clone', 'sync', 'skip')),
    entry_status        TEXT NOT NULL CHECK(entry_status IN ('running', 'success', 'failed')),
    bytes_transferred   INTEGER NOT NULL DEFAULT 0,
    new_commits         INTEGER NOT NULL DEFAULT 0,
    duration_ms         INTEGER,
    error_code          TEXT,
    error_message       TEXT,
    started_at          TEXT,
    completed_at        TEXT
);

-- Look up all entries for a specific run (Activity Log detail view)
CREATE INDEX idx_sync_entries_run ON sync_entries(run_id);

-- Look up sync history for a specific repo
CREATE INDEX idx_sync_entries_repo ON sync_entries(repo_id);

-- Filter failed entries in a run
CREATE INDEX idx_sync_entries_failed ON sync_entries(run_id, entry_status)
    WHERE entry_status = 'failed';
```

### 2.11 `sync_log`

Structured log lines produced during a sync. More granular than `sync_entries` — captures intermediate states, warnings, and informational messages. Used for the live log in Sync Center and the detailed view in Activity Log.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `run_id` | INTEGER | NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE | |
| `repo_id` | INTEGER | REFERENCES repositories(id) ON DELETE SET NULL | NULL for run-level messages |
| `level` | TEXT | NOT NULL DEFAULT 'info' CHECK(level IN ('debug','info','warn','error')) | |
| `message` | TEXT | NOT NULL | Human-readable log message |
| `timestamp` | TEXT | NOT NULL | ISO 8601 with millisecond precision |

```sql
CREATE TABLE sync_log (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id    INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    repo_id   INTEGER REFERENCES repositories(id) ON DELETE SET NULL,
    level     TEXT NOT NULL DEFAULT 'info'
                  CHECK(level IN ('debug', 'info', 'warn', 'error')),
    message   TEXT NOT NULL,
    timestamp TEXT NOT NULL
);

CREATE INDEX idx_sync_log_run ON sync_log(run_id, timestamp);
CREATE INDEX idx_sync_log_repo ON sync_log(repo_id, timestamp);
```

**Log retention:** Rows older than the most recent 500 sync runs are deleted by a periodic cleanup query. This prevents unbounded growth.

### 2.12 `settings`

Key-value store for all application settings. Persisted across sessions.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `key` | TEXT | PRIMARY KEY | Dot-notation setting path (e.g., "sync.default_concurrency") |
| `value` | TEXT | NOT NULL | String representation; parsed by application layer |
| `updated_at` | TEXT | NOT NULL DEFAULT (datetime('now')) | |

```sql
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Seed data — defaults inserted at migration time:**

```sql
INSERT OR IGNORE INTO settings (key, value) VALUES
    -- Sync defaults
    ('sync.clone_path',              ''),
    ('sync.default_concurrency',     '8'),
    ('sync.default_protocol',        'ssh'),
    ('sync.auto_prune',              'false'),
    ('sync.include_wikis',           'false'),
    ('sync.lfs_enabled',             'false'),

    -- Explorer defaults
    ('explorer.default_source',      'my_repos'),
    ('explorer.skip_archived',       'true'),
    ('explorer.skip_forks',          'true'),
    ('explorer.sort_column',         '2'),
    ('explorer.sort_ascending',      'true'),

    -- Appearance
    ('appearance.color_scheme',      'default'),
    ('appearance.compact_mode',      'false'),
    ('appearance.show_icons',        'true'),
    ('appearance.show_breadcrumbs',  'true'),

    -- Active account (for multi-account future)
    ('auth.active_account_id',       ''),
    ('auth.auto_validate_on_start',  'true');
```

**Setting keys follow a hierarchical dot notation.** The application layer is responsible for parsing values into the correct Rust types. No type information is stored in the database — this keeps the schema simple and avoids the complexity of a typed key-value store.

---

## 3. Convenience Views

Views simplify the most common queries. They are read-only from the application's perspective — all writes go through the base tables.

### 3.1 `repo_dashboard`

Pre-joins `repositories` + `local_clones` for Explorer and Dashboard rendering.

```sql
CREATE VIEW repo_dashboard AS
SELECT
    r.id,
    r.github_id,
    r.owner_login,
    r.name,
    r.full_name,
    r.description,
    r.language,
    r.default_branch,
    r.visibility,
    r.is_fork,
    r.is_archived,
    r.is_template,
    r.size_kb       AS remote_size_kb,
    r.stars,
    r.open_issues,
    r.license_spdx,
    r.topics_json,
    r.clone_url_ssh,
    r.clone_url_https,
    r.pushed_at,
    r.selected,
    r.deleted_on_remote,
    r.discovered_at,

    -- Local state (NULL if not cloned)
    lc.id           AS clone_id,
    lc.local_path,
    COALESCE(lc.clone_status, 'not_cloned') AS clone_status,
    lc.local_size_kb,
    lc.git_dir_size_kb,
    lc.behind_count,
    lc.ahead_count,
    lc.is_dirty,
    lc.current_branch,
    lc.clone_protocol,
    lc.last_synced_at,
    lc.last_error,
    lc.last_error_at,
    lc.consecutive_errors
FROM
    repositories r
    LEFT JOIN local_clones lc ON lc.repo_id = r.id;
```

**Why a view instead of querying the join directly:**
- Single import for both Explorer and Dashboard screens
- The LEFT JOIN is the correct semantics (repos without clones are included)
- If we later add denormalized columns to `repositories` for performance, the view shields the application from the change
- Rust's `rusqlite` can map view rows to the same struct as table rows

### 3.2 `sync_run_summary`

Joins `sync_runs` with aggregate data from `sync_entries` for the Activity Log list view.

```sql
CREATE VIEW sync_run_summary AS
SELECT
    sr.id,
    sr.started_at,
    sr.completed_at,
    sr.status,
    sr.trigger,
    sr.total_repos,
    sr.cloned_count,
    sr.updated_count,
    sr.skipped_count,
    sr.failed_count,
    sr.bytes_transferred,
    sr.duration_ms,
    sr.options_json,
    COUNT(se.id) FILTER (WHERE se.entry_status = 'failed') AS actual_failed_count
FROM
    sync_runs sr
    LEFT JOIN sync_entries se ON se.run_id = sr.id
GROUP BY sr.id;
```

---

## 4. Index Summary

All indexes and their query purposes:

| Index | Table | Purpose |
|-------|-------|---------|
| `idx_accounts_login` | github_accounts | Lookup account by username |
| `idx_orgs_login` | github_orgs | Lookup org by slug |
| `idx_repos_full_name` | repositories | Primary lookup: "does repo X exist?" (case-insensitive) |
| `idx_repos_owner` | repositories | Filter by owner in Explorer |
| `idx_repos_github_id` | repositories | Upsert during discovery (match by GitHub ID) |
| `idx_repos_visibility` | repositories | Filter by visibility |
| `idx_repos_fork` | repositories (partial) | Filter "skip forks" — only indexes forks |
| `idx_repos_archived` | repositories (partial) | Filter "skip archived" — only indexes archived |
| `idx_repos_discovered` | repositories | Prune candidates: repos not seen in recent discoveries |
| `idx_repos_pushed` | repositories | Sort by last push date |
| `idx_repos_stars` | repositories | Sort by star count |
| `idx_clones_status` | local_clones | Filter by clone status (Dashboard) |
| `idx_clones_behind` | local_clones (partial) | Dashboard attention list — only indexes stale repos |
| `idx_clones_error` | local_clones (partial) | Dashboard attention list — only indexes errored repos |
| `idx_sync_runs_started` | sync_runs | Activity Log: newest runs first |
| `idx_sync_runs_status` | sync_runs | Filter activity log by status |
| `idx_sync_entries_run` | sync_entries | All entries for a run (detail view) |
| `idx_sync_entries_repo` | sync_entries | Sync history for a specific repo |
| `idx_sync_entries_failed` | sync_entries (partial) | Failed entries in a run (post-sync error view) |
| `idx_sync_log_run` | sync_log | Log lines for a run, ordered by time |
| `idx_sync_log_repo` | sync_log | Log lines for a specific repo |

**Partial indexes rationale:** Indexes with `WHERE` clauses are smaller and faster to maintain. For example, `idx_repos_fork WHERE is_fork = 1` only indexes rows that are forks — which might be 10% of the table. The "skip forks" query benefits, but non-fork queries don't pay the index maintenance cost.

---

## 5. Migration Strategy

### 5.1 Forward-Only Migrations (Recommended)

Each migration is a numbered SQL file in `src/db/migrations/`:

```
src/db/migrations/
├── 001_initial_schema.sql
├── 002_add_topics_tables.sql
├── 003_add_github_accounts.sql
```

**Migration file format:**

```sql
-- Migration: 001_initial_schema
-- Description: Create core tables for repositories, sync history, and settings
-- Applied at: (auto-filled by migration runner)

-- Up
CREATE TABLE IF NOT EXISTS schema_version (...);
CREATE TABLE IF NOT EXISTS github_accounts (...);
CREATE TABLE IF NOT EXISTS repositories (...);
CREATE TABLE IF NOT EXISTS sync_runs (...);
CREATE TABLE IF NOT EXISTS sync_entries (...);
CREATE TABLE IF NOT EXISTS settings (...);

-- Seed defaults
INSERT OR IGNORE INTO settings (key, value) VALUES (...);

-- Record migration
INSERT INTO schema_version (version, name) VALUES (1, '001_initial_schema');
```

### 5.2 Migration Runner Algorithm

```
fn run_migrations(db: &Connection) -> Result<()> {
    // 1. Ensure schema_version table exists
    db.execute("CREATE TABLE IF NOT EXISTS schema_version (...)")?;

    // 2. Read current version
    let current_version: i64 = db.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )?;

    // 3. Read migration files from embedded directory (include_dir! macro)
    let migrations = load_migration_files();

    // 4. Apply pending migrations in order
    for migration in migrations {
        if migration.version > current_version {
            db.execute_batch(&migration.sql)?;
            log::info!("Applied migration {}: {}", migration.version, migration.name);
        }
    }

    Ok(())
}
```

### 5.3 Migration Rules

1. **Migrations are immutable.** Once a migration has been applied and committed, its SQL file is never modified. New changes get a new migration file with a higher version number.
2. **No down migrations.** Keeping rollback scripts in sync with forward migrations is error-prone. If a migration fails, the transaction is rolled back and the user sees an error. The fix is a new forward migration.
3. **Each migration runs in a transaction.** SQLite's `BEGIN` / `COMMIT` ensures atomicity. If a migration fails mid-way, the database is unchanged.
4. **`IF NOT EXISTS` is mandatory.** All `CREATE TABLE` statements use `IF NOT EXISTS` so migrations are idempotent. This also covers the edge case where the `schema_version` table was created but the version wasn't recorded (crash recovery).
5. **`INSERT OR IGNORE` for seed data.** Settings defaults may already exist from a previous partial migration.
6. **Version numbers are sequential integers** starting from 1. No gaps. No UUIDs. This ensures deterministic ordering.

### 5.4 Startup Flow

```
App startup
  │
  ├── Open SQLite connection
  ├── Enable WAL mode: PRAGMA journal_mode=WAL
  ├── Enable foreign keys: PRAGMA foreign_keys=ON
  ├── Run pending migrations
  │     ├── If schema_version table doesn't exist → create + apply all
  │     ├── If current version < latest → apply pending in order
  │     └── If current version == latest → no-op
  ├── Migrations complete
  └── Proceed to main loop
```

### 5.5 WAL Mode

SQLite Write-Ahead Logging is enabled at connection time:

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;
```

**Why WAL:**
- Concurrent reads and writes (readers don't block writers)
- Better performance for our workload (frequent writes during sync, frequent reads for UI)
- Default in modern SQLite but we set it explicitly for clarity

**Why `busy_timeout=5000`:**
- In WAL mode, writers may encounter busy conditions. A 5-second timeout lets them retry instead of failing immediately.
- This is important during sync when multiple Tokio tasks write to the database concurrently.

---

## 6. Query Patterns (for Service Layer Reference)

These are the SQL queries the service layer will use most frequently. Documented here for schema validation — every query must be supported by the indexes above.

### Discovery Upsert (most frequent write during discovery)

```sql
-- Insert or update a repository discovered from GitHub API
INSERT INTO repositories (
    github_id, owner_login, name, full_name, description, language,
    default_branch, visibility, is_fork, is_archived, is_template,
    size_kb, stars, open_issues, license_spdx, topics_json,
    clone_url_ssh, clone_url_https, homepage_url,
    pushed_at, created_at_gh, updated_at_gh,
    discovered_at, updated_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
ON CONFLICT(github_id) DO UPDATE SET
    owner_login = excluded.owner_login,
    name = excluded.name,
    full_name = excluded.full_name,
    description = excluded.description,
    language = excluded.language,
    default_branch = excluded.default_branch,
    visibility = excluded.visibility,
    is_fork = excluded.is_fork,
    is_archived = excluded.is_archived,
    is_template = excluded.is_template,
    size_kb = excluded.size_kb,
    stars = excluded.stars,
    open_issues = excluded.open_issues,
    license_spdx = excluded.license_spdx,
    topics_json = excluded.topics_json,
    clone_url_ssh = excluded.clone_url_ssh,
    clone_url_https = excluded.clone_url_https,
    homepage_url = excluded.homepage_url,
    pushed_at = excluded.pushed_at,
    updated_at_gh = excluded.updated_at_gh,
    deleted_on_remote = 0,
    discovered_at = datetime('now'),
    updated_at = datetime('now');
```

### Explorer Query (most frequent read)

```sql
SELECT * FROM repo_dashboard
WHERE
    (owner_login = ? OR ? IS NULL)     -- org filter
    AND (visibility = ? OR ? IS NULL)  -- visibility filter
    AND (is_fork = 0 OR ? = 0)         -- skip forks toggle
    AND (is_archived = 0 OR ? = 0)     -- skip archived toggle
    AND (full_name LIKE '%' || ? || '%' OR ? IS NULL)  -- text filter
    AND (deleted_on_remote = 0)
ORDER BY
    CASE WHEN ? = 1 THEN name END ASC,   -- sort by column index
    CASE WHEN ? = 2 THEN pushed_at END DESC,
    CASE WHEN ? = 3 THEN stars END DESC
LIMIT ? OFFSET ?;
```

### Dashboard Stats (aggregation)

```sql
SELECT
    COUNT(*) AS total_repos,
    COUNT(*) FILTER (WHERE clone_status = 'cloned') AS up_to_date,
    COUNT(*) FILTER (WHERE clone_status = 'stale') AS behind,
    COUNT(*) FILTER (WHERE clone_status = 'error') AS errors,
    COUNT(*) FILTER (WHERE clone_id IS NULL) AS not_cloned,
    COALESCE(SUM(local_size_kb), 0) + COALESCE(SUM(git_dir_size_kb), 0) AS total_size_kb
FROM repo_dashboard;
```

### Attention List (Dashboard)

```sql
SELECT full_name, owner_login, clone_status, behind_count, last_error, last_synced_at
FROM repo_dashboard
WHERE clone_status IN ('stale', 'error')
ORDER BY
    CASE WHEN clone_status = 'error' THEN 0 ELSE 1 END,  -- errors first
    behind_count DESC
LIMIT 10;
```

### Activity Log (sync history)

```sql
SELECT * FROM sync_run_summary
ORDER BY started_at DESC
LIMIT ? OFFSET ?;
```

### Activity Log Detail (entries for one run)

```sql
SELECT
    se.*,
    r.full_name,
    r.owner_login
FROM sync_entries se
JOIN repositories r ON r.id = se.repo_id
WHERE se.run_id = ?
ORDER BY se.started_at;
```

### Settings Read/Write

```sql
-- Read single setting
SELECT value FROM settings WHERE key = ?;

-- Write/update setting
INSERT INTO settings (key, value, updated_at)
VALUES (?, ?, datetime('now'))
ON CONFLICT(key) DO UPDATE SET
    value = excluded.value,
    updated_at = datetime('now');

-- Read all settings
SELECT key, value FROM settings;
```

### Prune Candidates

```sql
-- Repos deleted on remote but still cloned locally
SELECT * FROM repo_dashboard
WHERE deleted_on_remote = 1
  AND clone_id IS NOT NULL;
```

### Sync Log Cleanup

```sql
-- Delete log entries older than the most recent 500 sync runs
DELETE FROM sync_log
WHERE run_id NOT IN (
    SELECT id FROM sync_runs
    ORDER BY started_at DESC
    LIMIT 500
);
```

---

## 7. Performance Considerations

### Table Size Estimates (for a user with 500 repos, 200 sync runs)

| Table | Estimated rows | Row size | Total size |
|-------|---------------|----------|------------|
| github_accounts | 1 | ~300 B | <1 KB |
| github_orgs | 5-20 | ~300 B | <10 KB |
| org_memberships | 5-20 | ~100 B | <5 KB |
| repositories | 500 | ~800 B | ~400 KB |
| topics | ~200 | ~100 B | ~20 KB |
| repo_topics | ~1,500 | ~50 B | ~75 KB |
| local_clones | 50-200 | ~400 B | ~80 KB |
| sync_runs | 200 | ~300 B | ~60 KB |
| sync_entries | ~20,000 | ~300 B | ~6 MB |
| sync_log | ~50,000 | ~200 B | ~10 MB |
| settings | ~20 | ~100 B | <5 KB |
| **Total** | | | **~17 MB** |

**At 500 repos and 200 sync runs, the database is under 20 MB.** This is well within SQLite's comfort zone. SQLite performs excellently up to several GB.

### Write Patterns

- **Discovery:** Bulk upsert (1 write per repo, 100-500 writes in a few seconds). Wrapped in a single transaction for speed.
- **Sync progress:** Writes to `sync_entries` and `sync_log` trickle in over the sync duration (1 write per repo per ~10s during clone). Not bursty.
- **Settings:** Rare writes (user changes a setting). Not performance-sensitive.
- **Stats refresh:** Updates to `local_clones` rows. 50-200 writes per refresh. Wrapped in a transaction.

### Connection Pooling

For MVP, a single `rusqlite::Connection` wrapped in `Arc<Mutex<Connection>>` is sufficient. SQLite is single-writer by design. WAL mode allows readers to proceed during writes.

For future scaling (if concurrent DB access becomes a bottleneck), migrate to `r2d2-sqlite` with a pool size of 1-3. This amortizes the mutex contention but doesn't change the fundamental single-writer constraint.
