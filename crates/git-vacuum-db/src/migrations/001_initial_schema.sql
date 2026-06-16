-- Migration: 001_initial_schema
-- Description: Core tables for repositories, sync history, accounts, orgs, and settings

CREATE TABLE IF NOT EXISTS schema_version (
    version   INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS github_accounts (
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

CREATE INDEX IF NOT EXISTS idx_accounts_login ON github_accounts(login);

CREATE TABLE IF NOT EXISTS github_orgs (
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

CREATE INDEX IF NOT EXISTS idx_orgs_login ON github_orgs(login);

CREATE TABLE IF NOT EXISTS org_memberships (
    account_id  INTEGER NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
    org_id      INTEGER NOT NULL REFERENCES github_orgs(id) ON DELETE CASCADE,
    role        TEXT NOT NULL DEFAULT 'member' CHECK(role IN ('admin', 'member')),
    joined_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (account_id, org_id)
);

CREATE TABLE IF NOT EXISTS repositories (
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

CREATE INDEX IF NOT EXISTS idx_repos_full_name ON repositories(full_name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_repos_owner ON repositories(owner_login);
CREATE INDEX IF NOT EXISTS idx_repos_github_id ON repositories(github_id);
CREATE INDEX IF NOT EXISTS idx_repos_visibility ON repositories(visibility);
CREATE INDEX IF NOT EXISTS idx_repos_fork ON repositories(is_fork) WHERE is_fork = 1;
CREATE INDEX IF NOT EXISTS idx_repos_archived ON repositories(is_archived) WHERE is_archived = 1;
CREATE INDEX IF NOT EXISTS idx_repos_discovered ON repositories(discovered_at);
CREATE INDEX IF NOT EXISTS idx_repos_pushed ON repositories(pushed_at);
CREATE INDEX IF NOT EXISTS idx_repos_stars ON repositories(stars);

CREATE TABLE IF NOT EXISTS topics (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS repo_topics (
    repo_id  INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
    PRIMARY KEY (repo_id, topic_id)
);

CREATE TABLE IF NOT EXISTS local_clones (
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

CREATE INDEX IF NOT EXISTS idx_clones_status ON local_clones(clone_status);
CREATE INDEX IF NOT EXISTS idx_clones_behind ON local_clones(behind_count) WHERE behind_count > 0;
CREATE INDEX IF NOT EXISTS idx_clones_error ON local_clones(clone_status) WHERE clone_status = 'error';

CREATE TABLE IF NOT EXISTS sync_runs (
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

CREATE INDEX IF NOT EXISTS idx_sync_runs_started ON sync_runs(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_runs_status ON sync_runs(status);

CREATE TABLE IF NOT EXISTS sync_entries (
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

CREATE INDEX IF NOT EXISTS idx_sync_entries_run ON sync_entries(run_id);
CREATE INDEX IF NOT EXISTS idx_sync_entries_repo ON sync_entries(repo_id);
CREATE INDEX IF NOT EXISTS idx_sync_entries_failed ON sync_entries(run_id, entry_status)
    WHERE entry_status = 'failed';

CREATE TABLE IF NOT EXISTS sync_log (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id    INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    repo_id   INTEGER REFERENCES repositories(id) ON DELETE SET NULL,
    level     TEXT NOT NULL DEFAULT 'info'
                  CHECK(level IN ('debug', 'info', 'warn', 'error')),
    message   TEXT NOT NULL,
    timestamp TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sync_log_run ON sync_log(run_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_sync_log_repo ON sync_log(repo_id, timestamp);

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('sync.clone_path', ''),
    ('sync.default_concurrency', '8'),
    ('sync.default_protocol', 'ssh'),
    ('sync.auto_prune', 'false'),
    ('sync.include_wikis', 'false'),
    ('sync.lfs_enabled', 'false'),
    ('explorer.default_source', 'my_repos'),
    ('explorer.skip_archived', 'true'),
    ('explorer.skip_forks', 'true'),
    ('explorer.sort_column', '2'),
    ('explorer.sort_ascending', 'true'),
    ('appearance.color_scheme', 'default'),
    ('appearance.compact_mode', 'false'),
    ('appearance.show_icons', 'true'),
    ('appearance.show_breadcrumbs', 'true'),
    ('auth.active_account_id', ''),
    ('auth.auto_validate_on_start', 'true');

INSERT INTO schema_version (version, name) VALUES (1, '001_initial_schema');
