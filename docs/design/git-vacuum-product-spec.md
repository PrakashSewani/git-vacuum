# Git-Vacuum — Product Specification

**Status:** Draft  
**Last Updated:** 2026-06-16  
**Platform:** GitHub (initially; GitLab, Bitbucket, Gitea as future providers)

---

## 1. Target Users

Git-Vacuum serves three distinct personas, prioritized by immediate addressability:

### Primary: The "Belt-and-Suspenders" Developer
- Individual developer or freelancer with 20–200 repositories across personal accounts and orgs.
- Wants a local safety net: "If GitHub goes down or my account gets compromised, I still have everything."
- Runs backups manually or via cron. Values visibility into what's backed up and what's stale.
- Technical but doesn't want to script their own backup solution. Prefers a dedicated tool they can trust.

### Secondary: The Onboarding/Offboarding Engineer
- Team lead or DevOps engineer managing repository access for a growing or changing team.
- Needs to clone all team repos when someone joins, or archive them when someone leaves.
- Values speed (concurrent clones) and completeness (wikis, LFS, all branches).
- Uses the tool sporadically — maybe once a month — so discoverability matters.

### Tertiary: The Code Searcher
- Security researcher, architect, or consultant who needs to grep/search across dozens of repos.
- Doesn't necessarily want backups — wants a quick local mirror of an org's codebase to run analysis tools against.
- Values filterability (skip forks, skip archived, regex match) and a clean directory layout.

---

## 2. Primary Use Cases

| Priority | Use Case | Persona |
|----------|----------|---------|
| P0 | Authenticate with GitHub via PAT or OAuth device flow | All |
| P0 | Discover all repositories the authenticated user owns or contributes to | All |
| P0 | Clone discovered repositories to a local directory (concurrent, with progress) | All |
| P0 | Incrementally sync existing clones (fetch + merge, don't re-clone) | Primary |
| P1 | Filter repositories before cloning (skip forks, skip archived, regex, topics) | Tertiary |
| P1 | Mirror-mode backup (bare clones with all refs, for disaster recovery) | Primary |
| P1 | Clone wikis alongside repositories | Primary |
| P2 | Schedule automatic syncs via OS scheduler (cron/launchd/Task Scheduler) | Primary |
| P2 | Export a list of all discovered repositories (JSON/CSV) | Secondary |
| P2 | Clone repositories from a specific GitHub organization | Secondary |

---

## 3. MVP Definition

The MVP is a **Ratatui TUI application** that does exactly three things and does them well:

1. **Auth Screen** — User enters a GitHub Personal Access Token (or completes OAuth device flow). Credential is stored locally (OS keyring or encrypted config file).

2. **Discovery + Selection Screen** — Git-Vacuum queries the GitHub API and lists every repository the authenticated user owns, contributes to, or has starred. The user sees them in a scrollable, filterable table (name, visibility, last pushed, size). They can toggle individual repos on/off and apply global filters (skip archived, skip forks).

3. **Clone/Sync Screen** — With a single action (Enter key), Git-Vacuum concurrently clones selected repos to `~/git-vacuum/<owner>/<repo>/`. Shows live progress per repo (cloning, fetching, done, error). On subsequent runs, existing repos are incrementally synced (git fetch + fast-forward) rather than re-cloned.

**MVP scope is deliberately narrow.** No scheduling, no multi-provider, no mirror mode, no stats — those are v1.0. The MVP validates that a TUI approach to this problem is compelling.

---

## 4. What Should NOT Be in MVP

| Excluded | Rationale |
|----------|-----------|
| Multi-provider support (GitLab, Bitbucket, Gitea) | Dilutes focus. GitHub-first validates the core loop. |
| Mirror/bare clone mode | Backup-specific. Core loop is normal clones + sync. |
| Scheduled/cron operations | OS handles scheduling. Built-in scheduling is v1.0 polish. |
| Git LFS support | Edge case for MVP. Most repos don't use LFS. |
| Wiki cloning | Nice-to-have, not core. |
| Stats/analytics CSV export | Ghorg already does this well. Investigate for v1.0. |
| Config file (YAML/TOML) | MVP uses command-line flags + environment variables only. |
| Docker image | v1.0 distribution concern. |
| HTTP server / reclone-server | Ghorg's advanced automation. Not for MVP. |
| Submodule cloning | Edge case. Default clone behavior (no --recursive) is fine. |

---

## 5. Version 1.0 Features

Everything in MVP, plus:

- **Config file support** (`~/.config/git-vacuum/config.toml`) for persistent settings (default clone path, default filters, concurrency).
- **Mirror mode** — `--mirror` flag clones repos as bare mirrors (`git clone --mirror`), suitable for disaster recovery.
- **Wiki cloning** — `--include-wikis` flag clones associated wiki repos.
- **Git LFS support** — `--lfs` flag fetches LFS objects during clone/sync.
- **Repository list export** — `--export json|tsv` outputs the discovered repo list for scripting.
- **Single-command mode** — `git-vacuum sync --token <token>` does auth + discover + clone/sync in one non-interactive pass (for cron/CI use).
- **Post-sync hooks** — `--on-complete <script>` runs a user script after sync completes (for notification webhooks, etc.).
- **Auto-pruning** — Optionally remove local repos that no longer exist on GitHub (`--prune`).
- **Pre-built binaries** for Linux (x86_64, arm64), macOS (x86_64, arm64), Windows (x86_64) via GitHub Releases.
- **Homebrew formula** for macOS.

---

## 6. Future Roadmap Features

Post-1.0, ordered by likely value:

### Phase 2: Multi-Provider
- GitLab support (personal + group repos)
- Bitbucket Cloud support
- Gitea/Forgejo support
- Unified provider abstraction in config

### Phase 3: Advanced Sync
- Smart sync scheduling built into the TUI (set interval, show next-run countdown)
- Conflict detection (local changes vs. remote — warn, don't silently overwrite)
- Partial mirror: sync only specific branches, not all refs
- Disk usage dashboard (total size, per-repo size, growth over time)

### Phase 4: Ecosystem Integration
- `gh` CLI integration (use `gh auth token` if available, fall back to built-in auth)
- Notifications (desktop notification on sync complete, or webhook)
- GitHub Actions mode (run git-vacuum as a scheduled action to back up to S3/GCS)
- Repository metadata backup (issues, PRs, releases — not just git data)

### Phase 5: Power User
- Custom clone scripts per repo (pre-clone, post-clone hooks)
- Repository health scoring (stale forks, unmaintained repos flagged in TUI)
- Team mode — discover repos across all members of a GitHub org
- diff-snapshot — show what changed since last sync in the TUI

---

## 7. Competitive Analysis

### Direct Competitors

| Tool | Language | Stars | Scope | TUI? | Key Strength | Key Weakness |
|------|----------|-------|-------|------|-------------|-------------|
| **[ghorg](https://github.com/gabrie30/ghorg)** | Go | 2,100+ | Multi-SCM | No | Most mature; reclone automation; stats tracking; 5 SCM providers | Pure CLI, no interactivity; complex config surface; Go, not Rust |
| **[gitbackup](https://github.com/amitsaha/gitbackup)** | Go | 230 | Multi-SCM | No | GitHub Migration API support; OAuth device flow; clean config | Backup-only focus; smaller community; Go |
| **[github-backup](https://github.com/fauzaanu/github-backup)** | Go | 0 (new) | GitHub-only | No | Atomic staging; incremental fetch; well-structured Go code | Very new; single-provider; pure CLI |
| **[ghrip](https://github.com/GitHubToolbox/github-ripper)** | Ruby | 3 | GitHub-only | No | Simple API; dry-run mode | Archived since Nov 2025; Ruby runtime dependency |

### Indirect/Adjacent Tools

| Tool | Relationship |
|------|-------------|
| `gh repo clone` (GitHub CLI) | Clones one repo at a time. No bulk operations. Git-Vacuum complements it. |
| `gh repo list` + `xargs git clone` | The bash one-liner many devs use. Git-Vacuum replaces this with a proper tool. |
| `gitea dump` / GitLab backup rake | Server-side backup tools for admins. Git-Vacuum is client-side for users. |

### Competitive Landscape Summary

The market has **mature, capable CLI tools** (ghorg dominates) but **zero TUI tools**. Every existing tool follows the "fire and forget" pattern: type a command, wait for output, done. There's no interactive exploration, no visual progress, no dashboard.

---

## 8. Differentiators

Git-Vacuum's competitive position rests on **four pillars**, all enabled by the Ratatui TUI:

### 1. Interactive Discovery (Primary Differentiator)
No existing tool lets you *browse* your repositories before cloning. You type a command and hope you got the filters right. Git-Vacuum shows you exactly what will be cloned, lets you toggle repos on/off with visual feedback, and only then executes. This transforms "batch clone" from a leap of faith into a deliberate action.

### 2. Live Progress Visualization
Existing tools output text logs. Git-Vacuum shows a progress dashboard: per-repo status bars, overall completion percentage, transfer rate, and errors surfaced inline. For 100+ repo syncs, this is the difference between staring at a wall of text and understanding what's happening.

### 3. Persistent Local Dashboard
After cloning, the TUI becomes a management dashboard. It shows which repos are up-to-date, which have new commits on remote, and which had errors on the last sync. This gives the user ongoing awareness, not just a one-shot operation.

### 4. Rust-Native, Single Binary
Rust compiles to a single static binary with no runtime dependencies. Compared to Go tools (which are also single-binary), Rust offers stronger memory safety and a growing ecosystem of high-quality terminal libraries (Ratatui, crossterm). For a tool that handles concurrent network I/O and filesystem operations, Rust's ownership model is a genuine advantage — it eliminates entire classes of concurrency bugs.

### Bonus: The "Vacuum" Metaphor
The name suggests effortless cleanup. The TUI reinforces this: you see the mess (uncloned repos), press a button, and watch it get cleaned up. This is more memorable and satisfying than dry CLI output.

---

## 9. UX Principles

These principles guide every design decision in the TUI:

### Principle 1: Progressive Disclosure
Show the minimum necessary at each step. The auth screen doesn't show clone options. The discovery screen doesn't show progress bars until the user commits to cloning. Each screen has one job.

### Principle 2: Safe by Default
Git-Vacuum never modifies or deletes local repositories without explicit user action. Clones are always *additive*. Syncs are always *fetch + fast-forward* (no force push, no clean, no reset). Mirror mode requires an explicit `--mirror` flag. Pruning requires an explicit `--prune` flag.

### Principle 3: Keyboard-First, Discoverable
Every action has a keyboard shortcut shown on screen. The bottom bar shows available commands with their keys. The app should be fully usable without touching a mouse. Tab/Shift+Tab navigates between panels. `/` starts filtering. `Space` toggles selection. `Enter` confirms.

### Principle 4: Fast Perceived Performance
Concurrent operations with live feedback feel faster than faster sequential operations with no feedback. Show progress immediately, even if it's optimistic. Use spinners for indeterminate states. Never freeze the UI during network calls.

### Principle 5: Offline-Aware
The TUI should work meaningfully without a network connection. Show cached repository data. Allow browsing and inspecting local repos. Clearly indicate when data might be stale.

### Principle 6: Respect the Terminal
Don't assume 256-color support. Use a color scheme that works on light and dark backgrounds. Respect terminal dimensions — use scrollable lists, not fixed-size layouts. Support terminal resizing gracefully.

---

## 10. Information Architecture

### Screen Structure

```
┌─────────────────────────────────────────────────────────┐
│  Git-Vacuum                          [Tab: Next Screen] │  ← Title bar
├─────────────────────────────────────────────────────────┤
│                                                         │
│                    MAIN CONTENT AREA                     │  ← Changes per screen
│                                                         │
│                                                         │
├─────────────────────────────────────────────────────────┤
│  [Status Message / Context Help]                        │  ← Status bar
│  F1:Help  F2:Screen1  F3:Screen2  ...  q:Quit          │  ← Command bar
└─────────────────────────────────────────────────────────┘
```

### Screen Flow

```
 ┌──────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
 │  AUTH    │────▶│  DISCOVERY   │────▶│  PROGRESS    │────▶│  DASHBOARD   │
 │  SCREEN  │     │  & SELECT    │     │  & SYNC      │     │  (RESULTS)   │
 └──────────┘     └──────────────┘     └──────────────┘     └──────────────┘
       │                 │                     │                    │
       │                 │                     │                    │
       ▼                 ▼                     ▼                    ▼
  Tab navigates between screens (and cycles back from Dashboard to Discovery)
```

### Screen 1: Auth

**Purpose:** Get credentials and validate them.

**Content:**
- Input field for GitHub Personal Access Token (masked, like password entry)
- "Use OAuth Device Flow" option (opens browser, polls for token)
- "Use gh CLI token" option (reads `gh auth token` output)
- Status: "Authenticating..." → "Connected as @username" or error message
- If already authenticated (token stored in keyring), skip to Discovery automatically

**Edge cases:**
- Token has insufficient scopes → Show which scopes are missing (`repo` scope required)
- Network error → Retry with backoff, show error details on demand
- Token expired/revoked → Clear stored token, return to auth screen

### Screen 2: Discovery & Selection

**Purpose:** Show what's available and let the user choose what to clone.

**Content (3 panels, resizable):**

**Left panel — Source selector:**
- Radio buttons: "My Repos" | "Org Repos" | "Starred" | "All Accessible"
- If "Org Repos" selected, show text input for org name
- Filter bar below: text input with `/` shortcut

**Center panel — Repository list (main focus):**
- Scrollable table with columns: [✓] | Name | Owner | Visibility | Last Push | Size
- `Space` toggles the checkbox for selected row
- `Ctrl+A` selects all, `Ctrl+D` deselects all
- Color coding: green = selected, dim = filtered out, red = clone error (from previous run)
- Sortable by clicking column headers (keyboard: `1`=Name, `2`=Owner, etc.)

**Right panel — Details:**
- Selected repo details: description, language, stars, default branch, clone URL
- Quick stats: "47 of 128 repos selected (36%)"

**Global filters (toggle bar above table):**
- [ ] Skip archived repositories
- [ ] Skip forks
- [ ] Match regex: `_________`
- [ ] Filter by topic: `_________`

**Bottom command bar:** `Enter:Clone Selected | Tab:Next | /:Filter | Space:Toggle | Ctrl+A:All | q:Quit`

### Screen 3: Progress & Sync

**Purpose:** Execute the clone/sync operation with live feedback.

**Content (2 panels):**

**Top panel — Overall progress:**
- Progress bar: `[████████░░░░░░░░░░] 47/128 repos (8.2 GB / 31.5 GB)`
- Throughput: `↑ 42.3 MB/s | 22 active connections`
- Elapsed time: `00:04:32` | Estimated remaining: `00:08:15`
- Status summary: `47 done | 68 pending | 3 syncing | 0 failed`

**Bottom panel — Per-repo log (scrollable):**
```
✓ user/repo-one           cloned (12 MB, 0:03)
✓ user/repo-two           synced (+14 commits, 0:08)
⣾ org/big-repo            cloning... (45%, 1.2 GB / 2.7 GB)
⣾ org/another             fetching... (22 MB)
✗ user/deleted-repo       ERROR: repository not found (404)
  user/pending-repo       queued...
```
Each line is color-coded and updates in real time.

**Post-completion:**
- Summary popup: "128 repos processed: 125 succeeded, 3 failed. Total: 31.5 GB in 12:47."
- "View Errors" button to filter the log to failed repos only
- `Enter` → proceed to Dashboard

### Screen 4: Dashboard

**Purpose:** Show the current state of all local repositories post-sync.

**Content (similar to Discovery but with sync status):**

| Column change from Discovery: Replace `[✓]` checkbox with status icon. |
|---|

**Status icons:**
- ✓ Green — Up to date (synced successfully, no remote changes)
- ↑ Yellow — Behind remote (new commits available since last sync)
- ✗ Red — Error on last sync attempt
- ⬡ Gray — Not yet cloned (present on GitHub but not local)
- ⬢ Dim — Local only (no matching GitHub repo — was it deleted?)

**Actions available from Dashboard:**
- `s` — Sync selected repos (return to Progress screen)
- `r` — Re-discover from GitHub (return to Discovery screen)
- `p` — Prune local repos with no GitHub counterpart (with confirmation dialog)
- `o` — Open repo in `$EDITOR` or file manager
- `e` — Export current state as JSON

### Non-Screen Components

**Help Overlay (`F1` or `?` from any screen):**
Full keyboard shortcut reference, shown as a modal overlay.

**Error Dialog:**
Modal popup for fatal errors (network down, disk full, auth revoked). Shows error details, recovery suggestion, and Retry/Abort buttons.

**Confirmation Dialog:**
Used before destructive actions (prune, delete local repo). Requires explicit `y`/`n` or typing "yes" for high-risk operations.

---

## Appendix: Key Design Decisions (for Architecture Phase)

These are product-level decisions that will constrain architecture:

1. **GitHub-first, single-provider MVP.** Multi-provider abstraction is designed for but not built in MVP. The GitHub API client should be behind a trait from day one to avoid painting into a corner.

2. **Normal clones (not mirrors) in MVP.** Mirror mode (`git clone --mirror`) is a v1.0 feature. MVP uses standard clones because they're more useful for the browsing/searching use case.

3. **Concurrent clones in MVP.** The concurrent clone engine is core to the value proposition. It must be in MVP, not deferred.

4. **Keystore for credentials.** Use the OS keyring (via the `keyring` crate) for token storage. Fall back to an encrypted config file on platforms without keyring support.

5. **No config file in MVP.** Environment variables (`GIT_VACUUM_TOKEN`) and CLI flags only. Config file (`git-vacuum.toml`) arrives in v1.0. This simplifies the MVP surface.

6. **`~/git-vacuum/` as default clone path.** Following the convention of `~/ghorg/` and `~/.gitbackup/`. User-overridable via `--path` flag.

7. **No daemon/background process.** Git-Vacuum is a foreground TUI. Scheduling is delegated to OS tools (cron, launchd, Task Scheduler) triggered by a non-interactive `git-vacuum sync` command added in v1.0.
