-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Repository catalog (remote data cached from GitHub + local state)
CREATE TABLE IF NOT EXISTS repos (
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
    clone_url_https TEXT NOT NULL,
    size_kb         INTEGER,
    pushed_at       TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,

    clone_status    TEXT NOT NULL DEFAULT 'not_cloned'
                        CHECK(clone_status IN ('not_cloned','cloned','stale','error')),
    local_path      TEXT,
    local_size_kb   INTEGER,
    last_synced_at  TEXT,
    last_error      TEXT,
    behind_count    INTEGER NOT NULL DEFAULT 0,

    selected        INTEGER NOT NULL DEFAULT 1,
    discovered_at   TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_on_remote INTEGER NOT NULL DEFAULT 0,
    topics_json     TEXT
);

CREATE INDEX IF NOT EXISTS idx_repos_full_name ON repos(full_name);
CREATE INDEX IF NOT EXISTS idx_repos_owner ON repos(owner);
CREATE INDEX IF NOT EXISTS idx_repos_clone_status ON repos(clone_status);

-- Sync run history
CREATE TABLE IF NOT EXISTS sync_runs (
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

CREATE INDEX IF NOT EXISTS idx_sync_runs_started ON sync_runs(started_at DESC);

-- Per-repo results within a sync run
CREATE TABLE IF NOT EXISTS sync_entries (
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

CREATE INDEX IF NOT EXISTS idx_sync_entries_run ON sync_entries(run_id);

-- Application settings (key-value)
CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Authenticated accounts (metadata only — no token)
CREATE TABLE IF NOT EXISTS accounts (
    github_user_id  INTEGER PRIMARY KEY,
    login           TEXT NOT NULL,
    name            TEXT,
    email           TEXT,
    avatar_url      TEXT,
    scopes_json     TEXT NOT NULL DEFAULT '[]',
    token_expires_at TEXT,
    last_validated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed default settings
INSERT OR IGNORE INTO settings (key, value) VALUES
    ('clone_path', ''),
    ('default_concurrency', '8'),
    ('default_protocol', 'ssh'),
    ('skip_archived_default', 'true'),
    ('skip_forks_default', 'true'),
    ('auto_prune', 'false'),
    ('include_wikis', 'false'),
    ('lfs_enabled', 'false');
