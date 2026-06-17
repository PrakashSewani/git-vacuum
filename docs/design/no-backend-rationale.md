# Git-Vacuum — No-Backend Rationale

**Status:** Permanent architectural decision (load-bearing)
**Applies to:** All current and future versions (MVP, v1.0, Phase 2-5)
**Related:** [git-vacuum-architecture.md §0](./git-vacuum-architecture.md), [git-vacuum-product-spec.md §4](./git-vacuum-product-spec.md)

---

## 1. The Decision

Git-Vacuum has **no backend, no server, no proxy, and no cloud component**. The runtime topology is:

```
[TUI on user's machine]  ──HTTPS──▶  [GitHub REST API]
       │
       ├──▶ OS Keyring        (token)
       ├──▶ SQLite file       (catalog + history, no secrets)
       └──▶ ~/git-vacuum/     (filesystem, actual git clones)
```

This is not a deferral ("we'll add a server later"). It is a design constraint that constrains every feature decision in the project. When a feature sounds like it needs a backend, the answer is almost always "no — solve it locally" or "no — defer until user research demands it."

This document exists so that future contributors (and the author, six months from now) don't re-litigate the decision every time a new feature seems to call for a server.

---

## 2. Why No Backend (Rationale by Counter-Argument)

Each section answers a feature ask that *sounds* like it needs a backend, and explains why the local-only design handles it better.

### 2.1 "We need scheduled syncs" → Cron handles it
- **MVP/v1.0:** `git-vacuum sync --token <token>` non-interactive mode runs from `cron` / `launchd` / Task Scheduler. Already in the v1.0 spec (§5 of product-spec.md).
- **Why no daemon:** A long-running background process adds lifecycle complexity (autostart, crash recovery, log rotation) for no benefit — cron is the OS's job scheduler and already does this.
- **Future escape hatch:** If "smart sync" (Phase 3 of roadmap) ever needs sub-minute scheduling, a `--daemon` flag on the same binary can run it as a Tokio background loop in the user's session. Still no separate backend process.

### 2.2 "We need shared state across machines" → Out of scope, defer
- **Current need:** None. The primary persona is a single dev on one machine.
- **Why not solve now:** Cross-device sync needs an account system, conflict resolution, and a server. That's a 10x scope expansion for a feature zero users have asked for.
- **Future escape hatch:** If demand emerges, the design already supports it cleanly — `Database` is a trait. A `RemoteDatabase` impl (HTTPS-backed) can replace `SqliteDatabase` without touching the app/service layers. **Add this in Phase 4+ only if user research demands it.**

### 2.3 "We need to receive webhooks" → Discovery + pull is sufficient
- **What it would buy:** React to pushes instantly instead of polling on launch.
- **Why we don't need it:** GitHub's primary rate limit (5,000/hr for authenticated users) makes polling trivially cheap — a full discovery costs ~6 API calls. Webhooks require a public HTTPS endpoint, which contradicts the "local-only, no cloud account" promise.
- **Future escape hatch:** Webhooks would only become attractive if the user wanted *real-time* dashboards showing live push events. None of the current screens need this.

### 2.4 "We need a team mode for shared backup configs" → File-based config export
- **Phase 4 team mode** (per roadmap) doesn't need a server — it can be solved with a checked-in YAML config (`teams.yaml`) listing orgs and filter rules. Each team member runs their own git-vacuum against the same config. Decentralized, no server to maintain.

### 2.5 "We need a web UI" → TUI *is* the UI
- **Product differentiator:** The TUI is the moat. Adding a web UI splits dev effort and dilutes the "keyboard-first terminal-native" positioning (UX Principle 6 in product-spec.md).
- **If a web UI ever ships:** It should be a separate Rust binary (e.g., `git-vacuum-web`) using the same `git-vacuum-core` / `git-vacuum-db` crates. It would still talk directly to GitHub — still no intermediate backend.

---

## 3. The Triggers (None Apply Today)

A backend would be considered if and only if one of these triggers fires. None of them have.

| Trigger | What it would unlock | Threshold to act |
|---|---|---|
| Cross-device state divergence reports from real users | Desktop + laptop share selection, sync history, activity log | ≥3 distinct users in the wild request it within a 90-day window |
| Real-time push events needed for a UI | Dashboard shows "X new commits pushed in the last 60 seconds" | A documented screen in the product spec requires sub-minute freshness |
| Centralized team policy enforcement | A team admin defines "everyone backs up org Y with these filters" and individual machines inherit it | A paying team-customer asks for it |
| Compliance / audit requirement | Auditors need a server-side log of "who accessed which repo when" | A regulated-industry customer contracts for it |

When (if) one of these fires, the hexagonal architecture (traits at every infrastructure boundary) means the change is a swap-in, not a rewrite. `Database` → `RemoteDatabase` (HTTPS-backed), or a new `git-vacuum-web` binary sharing the `git-vacuum-core` and `git-vacuum-db` crates, would not require touching service or app layers.

---

## 4. The Hexagonal Escape Hatch

The architecture is designed so that "add a server" remains a localized change:

| Trait | Today's impl | Future server-backed impl | What would change |
|---|---|---|---|
| `Database` | `SqliteDatabase` (rusqlite) | `RemoteDatabase` (HTTPS to a server we operate) | `db/` module — service and app layers untouched |
| `KeyringStore` | `PlatformKeyring` (OS keyring) | `RemoteKeyringStore` (server-side vault) | `keyring/` module — service and app layers untouched |
| `GithubApi` | `OctocrabGithubApi` (direct) | Unchanged | Nothing — we still talk to GitHub directly |
| `GitOps` | `Git2GitOps` (libgit2) | Unchanged | Nothing — git operations are local by definition |

The composition root (`main.rs`) is the only place that wires concrete impls. Swapping any one is a single-file change.

---

## 5. What "No Backend" Means Concretely

For the implementer, "no backend" translates into a set of concrete invariants. These should hold at every commit:

### 5.1 Code invariants
- No outbound HTTP/HTTPS URLs in the codebase except:
  - `github/client.rs` → `https://api.github.com` (or configured enterprise URL)
  - OAuth device flow URLs (`github.com/login/device`, `github.com/login/oauth/access_token`)
- No server-socket code (`tokio::net::TcpListener`, `axum`, `actix`, `warp`, `hyper` server).
- No background process that outlives the TUI session — the `--daemon` flag (if added in Phase 3) is an in-process Tokio loop, not a separate service.
- No analytics, telemetry, or "phone home" code. Period.

### 5.2 Filesystem invariants
After a full sync run, the only files outside `~/git-vacuum/` that git-vacuum touched are:
- `~/.config/git-vacuum/config.toml` (optional, contains no secrets)
- `~/.local/share/git-vacuum/db.sqlite` (or platform equivalent; contains no secrets)
- One OS keyring entry: service `git-vacuum`, account `github-pat`

Verifiable with:
```bash
security find-generic-password -s git-vacuum        # macOS
cmdkey /list | findstr git-vacuum                    # Windows
secret-tool search service git-vacuum                # Linux
```

### 5.3 Token invariants
- The PAT/OAuth access token is written to **one place only**: `KeyringStore::set_token()`.
- It is read from the keyring on startup (validation task) and on demand (any API call).
- It appears in source code in **three places only**: `keyring/` module, `service/auth_service.rs`, `ui/components/input_field.rs` (the masked input widget).
- It is **never** in SQLite, **never** in a log line, **never** in a panic message, **never** in an error response.

These invariants are the implementation of the architecture. If a feature proposal would violate any of them, the proposal needs explicit review against the triggers in §3.

---

## 6. Verification (How to Audit a Build)

A reviewer can confirm the no-backend invariant holds with a small battery of checks:

```bash
# 1. No server-socket code
grep -rn "TcpListener\|TcpStream\|axum::\|actix\|warp::\|hyper::Server" src/

# 2. No outbound URLs except GitHub + OAuth
grep -rEn "https?://[^\"']+" src/ | grep -v "api.github.com\|github.com/login"

# 3. Token never in non-auth code
grep -rn "token" src/ | grep -v "keyring\|auth_service\|input_field"

# 4. No telemetry
grep -rEn "metrics\|telemetry\|analytics\|track_event" src/

# 5. Filesystem footprint after a real run
find ~ -newer /tmp/git-vacuum-test-marker \
    -path "*/git-vacuum/*" -o \
    -path "*/.config/git-vacuum/*" -o \
    -path "*/.local/share/git-vacuum/*" \
    2>/dev/null
# Should list only the three expected locations.
```

If any of these checks fail, it's a regression of the architectural decision and warrants reverting or explicit sign-off.

---

## 7. Open Questions

1. **Headless Linux fallback for the keyring** — should we fail loudly when Secret Service is unavailable, or fall back to `~/.config/git-vacuum/token.enc` encrypted with a machine-id-derived key? **Recommendation:** fail loudly. Silent fallback hides misconfiguration and creates a security-footgun surface (encrypted file is a downgrade from OS-managed secrets).
2. **Token metadata in SQLite** — confirm the `settings` table holds `github_username`, `scopes_json`, `token_expires_at` (none secret). **Alternative:** store nothing in SQLite and always read from keyring on startup. Slightly slower startup, but airtight (no metadata leak via DB dump).
3. **Multi-account support** — should the keyring support multiple `account = "github-pat-<user>"` entries? **Recommendation:** defer to v1.0+. Design the trait so it's possible (`KeyringStore::set_token_for_account(name, token)`) but don't ship the UI until a persona needs it.

---

## 8. Versioning This Decision

This document is a **load-bearing architecture decision**, not a design exploration. Changing it (adding a backend) requires:

1. One of the triggers in §3 actually firing (with evidence, not speculation).
2. A new ADR (Architecture Decision Record) in `docs/adr/` documenting the trigger, the alternatives considered, and why the local-only design is now insufficient.
3. Updates to [git-vacuum-architecture.md §0](./git-vacuum-architecture.md), [git-vacuum-product-spec.md §4](./git-vacuum-product-spec.md), and this file to reflect the new model.

Until all three happen, the local-only model is the model.
