# Git-Vacuum — GitHub Integration Layer Design

**Library:** Octocrab (async GitHub API client for Rust)  
**Authentication:** PAT (Personal Access Token) + OAuth Device Flow  
**Scope:** Repository discovery, organization enumeration, token validation  
**Non-scope:** The integration layer does NOT handle cloning, syncing, or local git operations — those belong to the `git/` module.

---

## 1. API Architecture

### 1.1 Layered Design

The GitHub integration is split into two layers within the `github/` module:

```
src/github/
├── mod.rs            # Re-exports, GithubApi trait definition
├── client.rs         # Octocrab instance builder, auth configuration, base URL
├── repos.rs          # Repository listing (owned, org, starred, all)
├── orgs.rs           # Organization enumeration + membership
├── user.rs           # Authenticated user profile (token validation)
├── auth.rs           # Device flow: init, poll, state machine
├── pagination.rs     # Paginated stream adapter with rate-limit awareness
├── rate_limit.rs     # Rate limit tracker, pre-flight checks, backoff
├── error.rs          # Typed error hierarchy
└── types.rs          # Domain types (RemoteRepo, UserInfo, OrgInfo, etc.)
```

**The `GithubApi` trait** is the boundary between service layer and infrastructure:

```
trait GithubApi: Send + Sync {
    // ── Auth ──
    fn set_token(&self, token: &str);
    async fn validate_token(&self) -> Result<UserInfo>;
    async fn device_flow_init(&self, client_id: &str, scopes: &[&str]) -> Result<DeviceFlowInit>;
    async fn device_flow_poll(&self, client_id: &str, device_code: &str) -> Result<DeviceFlowPoll>;

    // ── User ──
    async fn get_authenticated_user(&self) -> Result<UserInfo>;

    // ── Orgs ──
    async fn list_my_orgs(&self) -> Result<Vec<OrgInfo>>;

    // ── Repository Discovery ──
    async fn list_my_repos(&self) -> Result<PagedStream<RemoteRepo>>;
    async fn list_org_repos(&self, org: &str) -> Result<PagedStream<RemoteRepo>>;
    async fn list_starred_repos(&self) -> Result<PagedStream<RemoteRepo>>;
    async fn list_all_accessible_repos(&self) -> Result<PagedStream<RemoteRepo>>;  // search-based

    // ── Rate Limit ──
    async fn get_rate_limit(&self) -> Result<RateLimitStatus>;
    fn remaining_rate_limit(&self) -> RateLimitSnapshot;
}
```

### 1.2 Octocrab Configuration

The `client.rs` module builds and owns the `Octocrab` instance. This is done once at startup.

**Configuration inputs:**
- `base_url`: Defaults to `https://api.github.com`. Overridable for GitHub Enterprise (e.g., `https://github.mycompany.com/api/v3`).
- `user_agent`: Required by GitHub. Format: `git-vacuum/0.1.0` (version from Cargo.toml).
- `token`: Set after authentication completes. Not required at client construction time.
- `preview_headers`: For GitHub API features still in preview. None needed for MVP.

**Client construction flow:**

```
OctocrabBuilder::default()
    .base_uri(base_url)?                         // api.github.com or enterprise
    .add_header(USER_AGENT, user_agent)?         // required by GitHub
    .add_header(ACCEPT, "application/json")?     // prefer JSON responses
    .build()?
```

**Token attachment:** The token is attached per-request via Octocrab's auth mechanism. Octocrab supports `PersonalAccessToken` authentication natively. We use a custom auth layer that injects the `Authorization: Bearer <token>` header and handles the two cases:
- Token set after construction (user provides PAT or receives OAuth token)
- Token not set (pre-auth requests like device flow init — these use `client_id` + `client_secret` for the device flow endpoints)

**Why not use Octocrab's static `instance()` pattern?** The static global singleton makes testing difficult and prevents multiple connections (e.g., for GitHub Enterprise in the future). We create and own the `Octocrab` instance and pass it via the `GithubApi` trait.

### 1.3 Domain Types

These are our internal types, not Octocrab's `models::Repository`. We define our own to avoid coupling the entire codebase to Octocrab's model changes and to include only the fields we need.

```
struct UserInfo {
    github_user_id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
    scopes: Vec<String>,              // parsed from x-oauth-scopes header
    token_expires_at: Option<DateTime>,  // parsed from token introspection
}

struct OrgInfo {
    github_org_id: i64,
    login: String,
    display_name: Option<String>,
    description: Option<String>,
    avatar_url: Option<String>,
    repos_count: i32,
}

struct RemoteRepo {
    github_id: i64,
    owner_login: String,
    name: String,
    full_name: String,
    description: Option<String>,
    language: Option<String>,
    default_branch: String,
    visibility: RepoVisibility,
    is_fork: bool,
    is_archived: bool,
    is_template: bool,
    size_kb: Option<i64>,
    stars: i32,
    open_issues: i32,
    license_spdx: Option<String>,
    topics: Vec<String>,
    clone_url_ssh: Option<String>,
    clone_url_https: String,
    homepage_url: Option<String>,
    pushed_at: Option<DateTime>,
    created_at: DateTime,
    updated_at: DateTime,
    owner_is_org: bool,
}

enum RepoVisibility {
    Public,
    Private,
    Internal,
}

struct DeviceFlowInit {
    device_code: String,       // 40 chars, used for polling
    user_code: String,         // 8 chars (XXXX-XXXX), shown to user
    verification_uri: String,  // https://github.com/login/device
    expires_in: Duration,      // typically 900 seconds
    interval: Duration,        // minimum polling interval (typically 5 seconds)
}

enum DeviceFlowPoll {
    Pending,
    Success { access_token: String, scopes: Vec<String> },
    SlowDown { new_interval: Duration },     // rate-limited, adjust polling
    Expired,                                  // device_code expired, re-init
    AccessDenied,                             // user clicked cancel
}
```

**Mapping from Octocrab models → our types:** Each function in the `github/` module converts Octocrab's `models::Repository` into our `RemoteRepo`. This mapping lives in the `repos.rs` file, not leaked to callers.

---

## 2. Authentication Flow

### 2.1 Authentication Methods (three supported paths)

```
                    ┌──────────────────────────────────┐
                    │        Git-Vacuum Startup         │
                    └────────────────┬─────────────────┘
                                     │
                          Check: token in keyring?
                                     │
                    ┌────────────────┼────────────────┐
                    │ YES            │ NO             │
                    ▼                │                ▼
           ┌──────────────┐         │       ┌──────────────────┐
           │ Validate PAT  │         │       │  Auth Gate Screen │
           │ (GET /user)   │         │       └────────┬─────────┘
           └──────┬───────┘         │                │
                  │                 │     User chooses method:
         ┌────────┴────────┐        │     ┌──────────┼──────────┐
         │ VALID            │ INVALID│     │ PAT      │ OAuth    │ gh CLI  │
         ▼                  ▼        │     ▼          ▼          ▼
  ┌──────────────┐  ┌──────────────┐ │  ┌──────┐ ┌────────┐ ┌──────────┐
  │ Enter main   │  │ Show auth    │ │  │Enter │ │Device  │ │Read `gh  │
  │ loop         │  │ gate screen  │ │  │PAT   │ │Flow    │ │auth      │
  └──────────────┘  └──────────────┘ │  └──┬───┘ └───┬────┘ │token`    │
                                     │     │         │       └────┬─────┘
                                     │     └────┬────┴───────────┘
                                     │          │
                                     │          ▼
                                     │  ┌────────────────┐
                                     │  │ Store in keyring│
                                     │  │ Validate on     │
                                     │  │ GitHub (GET     │
                                     │  │ /user)          │
                                     │  └───────┬────────┘
                                     │          │
                                     │   ┌──────┴──────┐
                                     │   │ VALID?       │
                                     │   ▼              ▼
                                     │ Success      Show error,
                                     │              retry loop
                                     │
                                     └──────────────────┘
```

### 2.2 PAT (Personal Access Token) Flow

**Entry point:** User types token in the auth gate screen.

**Steps:**
1. User enters token (masked input field, validated locally for minimum length >0).
2. Token is passed to `GithubApi::validate_token()`.
3. `validate_token()` makes `GET /user` with `Authorization: Bearer <token>`.
4. On success (200): Parse `UserInfo` from response body. Extract OAuth scopes from `X-OAuth-Scopes` response header (comma-separated string). Cache token in OS keyring. Emit `AppEvent::AuthSucceeded`.
5. On failure (401, 403): Token is invalid, expired, or lacks scopes. Emit `AppEvent::AuthFailed` with reason.

**Scope validation:** The UI checks that the returned scopes include at minimum:
- `repo` — required for accessing private repositories
- `read:org` — required for listing organization memberships

If scopes are insufficient, the error message specifies which scopes are missing and links to the PAT creation page.

**Token storage:** See §2.6 below for the full contract. Summary: the raw token string is stored in the OS keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service / `libsecret`). It is NEVER stored in SQLite. The `settings` table stores only metadata (username, scopes, expiry) — none of which are secrets.

### 2.3 OAuth Device Flow

**Entry point:** User selects "OAuth Device Flow" in the auth gate screen.

**Precondition:** A GitHub OAuth App must be registered (either by the user or as a bundled app). The app needs:
- `client_id` — public, bundled in the binary or read from config
- `client_secret` — NOT bundled. Read from environment variable (`GIT_VACUUM_CLIENT_SECRET`) or config file. The device flow only strictly requires `client_id` (client_secret is optional for public clients).
- Device flow enabled in the app's settings

**Step 1: Initiate device flow**

```
POST https://github.com/login/device/code
  client_id=<CLIENT_ID>
  scope=repo read:org
```

Response:
```
{
  "device_code": "3584d83530557fdd1f46af8289938c8ef79f9dc5",
  "user_code": "WDJB-MJHT",
  "verification_uri": "https://github.com/login/device",
  "expires_in": 900,
  "interval": 5
}
```

The `user_code` is displayed prominently in the TUI. The `verification_uri` is shown as a URL. The user opens the URL, enters the code, and authorizes.

**Step 2: Display authorization prompt**

The TUI shows:
- The URL: `https://github.com/login/device`
- The user code: `WDJB-MJHT` (large, prominent, with a "Copy to clipboard" action)
- A countdown timer: "Code expires in 14:52"
- A status indicator: "Waiting for authorization..."

**Step 3: Poll for token**

The application polls `POST https://github.com/login/oauth/access_token` at the interval specified in step 1:

```
POST https://github.com/login/oauth/access_token
  client_id=<CLIENT_ID>
  device_code=<DEVICE_CODE>
  grant_type=urn:ietf:params:oauth:grant-type:device_code
```

**Polling state machine:**

```
┌─────────┐     poll(interval)     ┌──────────┐
│ PENDING │ ─────────────────────▶ │ PENDING  │ (authorization_pending)
└────┬────┘                        └──────────┘
     │                                   │
     │ poll(interval)                    │ poll(interval)
     ▼                                   ▼
┌──────────┐                        ┌──────────┐
│ SLOWDOWN │ ── adjust interval ──▶ │ PENDING  │
└──────────┘                        └──────────┘
     │
     │ poll(adjusted_interval)
     ▼
┌──────────┐     ┌──────────────┐     ┌───────────────┐
│ SUCCESS  │     │ EXPIRED      │     │ ACCESS_DENIED │
│ (token)  │     │ (re-init)    │     │ (user cancel) │
└──────────┘     └──────────────┘     └───────────────┘
```

**Error handling during polling:**

| GitHub Error | Our Handling |
|-------------|-------------|
| `authorization_pending` | Continue polling after `interval`. Update TUI spinner. |
| `slow_down` | Increase `interval` by 5 seconds. Update polling timer. Continue. |
| `expired_token` | Stop polling. Emit `AppEvent::OAuthTimeout`. UI shows "Code expired. Request a new code." |
| `access_denied` | Stop polling. Emit `AppEvent::AuthFailed { reason: "access_denied" }`. |
| `device_flow_disabled` | Fatal config error. Emit `AppEvent::AuthFailed { reason: "device_flow_disabled" }`. |
| Network error | Exponential backoff up to 3 retries. On permanent failure, show network error message. |

**Polling implementation detail:** The polling loop runs in a background Tokio task spawned by the `Effect::PollOAuthToken` effect. It sleeps for `interval` seconds between requests, checks `cancel_rx` before each poll, and sends events via `app_tx`.

**Maximum polling duration:** The loop terminates after `expires_in` seconds (900s by default) or when the user cancels. The countdown timer in the TUI is updated via `AppEvent` ticks.

### 2.4 gh CLI Token (Auto-detect)

**Entry point:** User selects "gh CLI Token" or the app auto-detects it.

**Steps:**
1. Run `gh auth token` as a subprocess (one-shot, synchronous, not repeated).
2. Parse stdout as the token string.
3. If successful, treat identically to a manually entered PAT: validate via `GET /user`, store in keyring.

**Fallback:** If `gh` is not installed or `gh auth token` fails, fall back to the PAT input screen.

### 2.5 Token Validation (Periodic)

After initial auth, the token is validated:
- On every app startup (if stored token exists)
- On every discovery run (as a pre-flight check)
- When the user clicks "Test Connection" in Settings

**Validation endpoint:** `GET /user` — lightweight, returns user identity. Also check `X-OAuth-Scopes` header to verify scopes haven't changed.

**Validation failure handling:**
- 401 Unauthorized → token expired or revoked. Clear keyring. Transition to Auth gate screen.
- 403 Forbidden → token lacks required scopes or account is suspended. Show specific error.
- Network timeout → retry with backoff. Show "Cannot reach GitHub" banner. App remains in cached/offline mode.

### 2.6 Token Storage Contract

This section is the authoritative spec for where the token lives, how it's protected, and how it gets re-validated. All other sections refer back here.

#### 2.6.1 Where the token goes

After `AuthSucceeded`, the single write site is `KeyringStore::set_token(token)`:

```
Effect::AuthenticatePat → validate via GET /user
                        → keyring.set_token(token)        // ← ONLY place token is persisted
                        → emit AppEvent::AuthSucceeded
```

| Store | Holds | Lifetime |
|---|---|---|
| **OS Keyring** (`service = "git-vacuum"`, `account = "github-pat"`) | The PAT or OAuth access token string | Until user runs `:auth logout` or token validation fails on 401 |
| **SQLite `settings` table** | `github_username`, `scopes_json`, `token_expires_at` (metadata only) | Persistent — used to show "Logged in as @user" on the auth gate without re-reading the keyring |
| **In-memory `App` / `Services`** | Token in `Arc<Services>` while the app runs | Dropped on quit |
| **Filesystem logs / SQLite row content** | **NEVER** the token | N/A |

#### 2.6.2 Keyring implementation (`keyring/` module)

The `KeyringStore` trait abstracts three platforms behind one API:

```
trait KeyringStore: Send + Sync {
    fn set_token(&self, token: &str) -> Result<()>;
    fn get_token(&self) -> Result<Option<String>>;
    fn delete_token(&self) -> Result<()>;
}
```

Concrete impls (per `keyring/platform.rs`):

- **macOS:** `security` CLI / Security.framework via the `keyring` crate. Service: `git-vacuum`, Account: `github-pat`.
- **Windows:** `wincred` (Credential Manager vault) via the `keyring` crate.
- **Linux:** `libsecret` (Secret Service / GNOME Keyring / KWallet) via the `keyring` crate. **Fallback policy:** if no Secret Service is available (headless server, no DBus session), the operation fails with a clear error rather than silently falling back to plaintext. Users who need a headless install can use the `--token <pat>` env-var or flag for that session.

#### 2.6.3 Token rotation & revocation flow

```
Startup:
  1. App::new() reads keyring → if token present, spawn validation task
  2. validate_token() → GET /user with Bearer token
  3a. 200 OK → parse X-OAuth-Scopes header, update SQLite metadata, transition to Running
  3b. 401 Unauthorized → keyring.delete_token(), transition to Auth gate, emit AppEvent::AuthFailed { reason: "expired_or_revoked" }
  3c. Network error → retry with backoff (3 attempts, 1s/2s/4s). If still failing, allow Running in offline mode (cached data only, no discovery).

Mid-session expiry (rare, mostly affects long-lived OAuth tokens):
  Any service-layer API call that returns 401 → emit AppEvent::AuthFailed → reducer transitions Running → Auth, clears in-memory token reference, shows modal "Your session expired. Please re-authenticate."
  Keyring is NOT auto-deleted on 401 — user might re-validate the same token from a different device.
```

#### 2.6.4 OAuth device flow specifics

OAuth tokens are stored identically to PATs — in the OS keyring under the same `service = "git-vacuum", account = "github-pat"` slot. The user experience is identical post-auth: they don't care whether they typed a PAT or completed device flow; the system stores the resulting access token the same way.

`client_secret` (if used for non-public OAuth clients) is read from:
- Env var `GIT_VACUUM_CLIENT_SECRET` first
- Config file `~/.config/git-vacuum/config.toml` second
- Bundled default for public-client device flow (no secret needed)

`client_secret` is **never** written to SQLite, never logged, never embedded in error messages.

#### 2.6.5 Security audit checklist (for the implementer)

- [ ] Grep the codebase for `token` — verify it only appears in `keyring/`, `service/auth_service.rs`, and `ui/components/input_field.rs` (for the masked input). Command: `grep -rn "token" src/ | grep -v keyring | grep -v auth_service.rs | grep -v input_field.rs` should return zero hits.
- [ ] Grep SQLite migrations and queries — no column named `token`, `access_token`, `pat`, `secret`.
- [ ] Grep log calls (`log::*!`) — no token interpolation. The `taste` file says minimize logging anyway.
- [ ] Token input field in TUI uses `mask_input: true` (already in the `Modal::InputPrompt` design).
- [ ] Error messages from `service/auth_service.rs` map `GithubError::Auth(InvalidToken)` to "Authentication failed" — never echo the token back.
- [ ] When the user types `Ctrl+C` during OAuth device flow polling, `cancel_tx` triggers and the polling task aborts — no partial token lingering.
- [ ] Token-never-logged test: `RUST_LOG=trace git-vacuum sync --token ghp_FAKEFAKEFAKEFAKEFAKE` (using a deliberately fake token). Grep stderr for the token string — must not appear.

---

## 3. Repository Discovery

### 3.1 Discovery Sources

The Explorer screen offers four discovery sources. Each maps to specific GitHub API calls:

| Source | API Endpoints | Pagination |
|--------|--------------|------------|
| **My Repos** | `GET /user/repos?affiliation=owner,collaborator,organization_member&sort=updated&per_page=100` | Yes |
| **Org Repos** | `GET /orgs/{org}/repos?type=all&sort=updated&per_page=100` | Yes |
| **Starred** | `GET /user/starred?sort=created&per_page=100` | Yes |
| **All Accessible** | Merge of My Repos + all Org Repos (discover orgs via `GET /user/orgs`, then fetch each org's repos) | Yes |

**"All Accessible" strategy:** This is the most expensive operation. It:
1. Calls `GET /user/orgs` to list all organizations the user belongs to.
2. Calls `GET /user/repos?affiliation=owner,collaborator,organization_member` for personal repos.
3. For each org, calls `GET /orgs/{org}/repos?type=all`.
4. Merges all results, deduplicating by `github_id`.

**Parallelism in multi-org discovery:** When fetching repos for multiple orgs, requests are made concurrently (up to 4 concurrent org fetches, respecting the secondary rate limit of 100 concurrent requests).

### 3.2 Pagination Strategy

GitHub's REST API uses **page-based pagination** with `Link` headers:

```
Link: <https://api.github.com/user/repos?page=2>; rel="next",
      <https://api.github.com/user/repos?page=10>; rel="last"
```

**Octocrab's built-in pagination:** Octocrab provides `all_pages::<T>(first_page)` which follows `Link` headers automatically. However, it warns "no rate limiting" — it will eagerly consume all pages back-to-back without any rate limit awareness.

**Our pagination adapter (`pagination.rs`):** We wrap Octocrab's paging in a `PagedStream<T>` that:
1. Fetches pages lazily (on-demand, not eagerly). The caller drives consumption.
2. Checks rate limit headers after every page response (`x-ratelimit-remaining`).
3. If remaining rate limit drops below a threshold (default: 50 requests), pauses and waits until the reset window (`x-ratelimit-reset`).
4. Implements exponential backoff on `429 Too Many Requests` responses (secondary rate limits).
5. Respects `cancel_rx` — stops fetching if the user cancels the discovery.

**PagedStream API (conceptual):**

```
// The stream yields batches of RemoteRepo (one page at a time).
// Consumed by service/discovery.rs which aggregates into Vec and caches in SQLite.

struct PagedStream<T> {
    current_page: Option<Page<T>>,       // Octocrab's Page struct
    next_url: Option<String>,           // parsed from Link header
    rate_limiter: RateLimiter,
    cancel_rx: watch::Receiver<bool>,
}

impl<T: DeserializeOwned> PagedStream<T> {
    async fn next_page(&mut self) -> Result<Option<Vec<T>>>;
    async fn collect_all(&mut self) -> Result<Vec<T>>;  // convenience for small result sets
    fn estimated_pages(&self) -> Option<usize>;  // from last page URL if available
}
```

**Page size:** We request `per_page=100` (GitHub's maximum). For 500 repos, this means 5 API calls. Well within the 5,000/hour primary rate limit.

### 3.3 Discovery Flow

The `DiscoverRepos` effect triggers `service::discovery::discover_repos()` which orchestrates:

```
discover_repos(token, source, github_api, db)
  │
  ├── 1. Validate token (GET /user) — fail fast if token invalid
  │
  ├── 2. Fetch remote repos based on source:
  │       ├── source == MyRepos → github_api.list_my_repos()
  │       ├── source == Org(name) → github_api.list_org_repos(name)
  │       ├── source == Starred → github_api.list_starred_repos()
  │       └── source == All → parallel: list_my_orgs() + list_my_repos() + list_org_repos(org) per org
  │           Stream pages through PagedStream
  │           Send progress events: AppEvent::DiscoveryProgress { repos_found: N }
  │
  ├── 3. Collect all RemoteRepo into Vec
  │
  ├── 4. Load existing repos from SQLite (all repos, not just this source)
  │
  ├── 5. Merge remote + cached data (see §5 Data Synchronization)
  │
  ├── 6. Mark repos NOT in the remote set as deleted_on_remote = 1
  │       (Only repos from the same owner scope — don't mark other owners' repos)
  │
  ├── 7. Batch upsert to SQLite (single transaction)
  │
  ├── 8. Emit AppEvent::ReposDiscovered { repos, source }
  │
  └── Return
```

**Discovery progress events:** For large orgs (1000+ repos), discovery can take several seconds. We emit periodic progress events so the Explorer shows a loading spinner and a "Found 237 repos so far..." counter, rather than appearing frozen.

### 3.4 API Endpoint Reference

| Operation | Method | Endpoint | Octocrab API | Notes |
|-----------|--------|----------|-------------|-------|
| Validate token / get user | GET | `/user` | `octocrab.current().user().await` | Returns `User` model |
| List my repos | GET | `/user/repos` | `octocrab.repos(owner, repo)` but we need the user-level endpoint. Use `octocrab.get::<Vec<Repository>>("/user/repos", params)?` with custom params | affiliation=owner,collaborator,organization_member |
| List org repos | GET | `/orgs/{org}/repos` | `octocrab.orgs(org).list_repos().send().await` | type=all (includes private if authorized) |
| List user orgs | GET | `/user/orgs` | `octocrab.orgs("").list_my_orgs()` — use `octocrab.current().list_orgs().await` (may need raw HTTP) | Returns all orgs the user belongs to |
| List starred repos | GET | `/user/starred` | `octocrab.activity().list_repos_starred_by_authenticated_user().send().await` | Sorted by starred date |
| Get rate limit | GET | `/rate_limit` | `octocrab.ratelimit().get().await` | Does not count against rate limit (primary), but counts against secondary |
| Device flow init | POST | `/login/device/code` | Custom — not in Octocrab's typed API. Use raw `octocrab._post()` | Requires client_id |
| Device flow poll | POST | `/login/oauth/access_token` | Custom — raw `octocrab._post()` | grant_type=urn:ietf:params:oauth:grant-type:device_code |

**Where Octocrab lacks typed endpoints:** For `/user/repos` (listing authenticated user's repos), Octocrab's `repos` module is repo-specific (requires owner+repo). The user-level repos listing and device flow endpoints require raw HTTP calls via `octocrab._get()` / `octocrab._post()`. We build typed wrappers around these in `repos.rs` and `auth.rs`.

### 3.5 API Response → Domain Type Mapping

GitHub's `Repository` model has ~80 fields. We extract ~22 into `RemoteRepo`. The mapping function is in `repos.rs`:

```
fn map_github_repo(r: octocrab::models::Repository) -> RemoteRepo {
    RemoteRepo {
        github_id: r.id.into_inner(),
        owner_login: r.owner.map(|o| o.login).unwrap_or_default(),
        name: r.name,
        full_name: r.full_name.unwrap_or_default(),
        description: r.description,
        language: r.language.map(|l| l.to_string()),
        default_branch: r.default_branch.unwrap_or_else(|| "main".into()),
        visibility: match r.visibility {
            Some(v) if v == "private" => RepoVisibility::Private,
            Some(v) if v == "internal" => RepoVisibility::Internal,
            _ => RepoVisibility::Public,
        },
        is_fork: r.fork.unwrap_or(false),
        is_archived: r.archived.unwrap_or(false),
        is_template: r.is_template.unwrap_or(false),
        size_kb: r.size,
        stars: r.stargazers_count.unwrap_or(0),
        open_issues: r.open_issues_count.unwrap_or(0),
        license_spdx: r.license.and_then(|l| l.spdx_id),
        topics: r.topics.unwrap_or_default(),
        clone_url_ssh: r.ssh_url,
        clone_url_https: r.clone_url.unwrap_or_default(),
        homepage_url: r.homepage,
        pushed_at: r.pushed_at,
        created_at: r.created_at,
        updated_at: r.updated_at,
        owner_is_org: r.owner.map(|o| o.type_ == "Organization").unwrap_or(false),
    }
}
```

---

## 4. Rate Limiting

### 4.1 Rate Limit Model

GitHub has two rate limit tiers:

**Primary rate limit (per-user, per-hour):**
| Auth method | Limit | Window |
|-------------|-------|--------|
| Unauthenticated | 60 | 1 hour |
| PAT / OAuth token | 5,000 | 1 hour |
| GitHub App (installation) | 5,000–12,500 | 1 hour |

Git-Vacuum always uses authenticated requests, so the effective limit is **5,000 requests/hour** (~83/minute, 1.4/second).

**Secondary rate limit (per-route, concurrent):**
- Max 100 concurrent requests across all endpoints
- Max 900 points/minute for REST API (most GET = 1 point)
- Max 90 seconds CPU time per 60 seconds real time

### 4.2 Rate Limit Tracker (`rate_limit.rs`)

The `RateLimiter` struct is shared across all GitHub API calls. It tracks the current rate limit state without making additional API calls — it reads the response headers from every API response.

```
struct RateLimiter {
    // State read from x-ratelimit-* response headers (updated after every API call)
    limit: AtomicU32,         // x-ratelimit-limit (5,000)
    remaining: AtomicU32,     // x-ratelimit-remaining
    reset_at: AtomicI64,      // x-ratelimit-reset (Unix epoch seconds)
    resource: AtomicStr,      // x-ratelimit-resource ("core", "search", etc.)

    // Pre-flight check thresholds
    warn_threshold: u32,      // Log warning when remaining < this (default: 100)
    pause_threshold: u32,     // Pause API calls when remaining < this (default: 50)

    // Secondary rate limit tracking
    concurrent_requests: AtomicU32,   // Current in-flight requests
    max_concurrent: u32,              // Max concurrent (default: 20, well below 100)
    last_429_at: AtomicI64,           // Last time we got a 429
    backoff_multiplier: AtomicU32,    // Exponential backoff multiplier
}
```

**Header parsing (after every API response):**
```
fn update_from_response(&self, headers: &HeaderMap) {
    if let Some(v) = headers.get("x-ratelimit-remaining") {
        self.remaining.store(v.parse().unwrap_or(0));
    }
    if let Some(v) = headers.get("x-ratelimit-reset") {
        self.reset_at.store(v.parse().unwrap_or(0));
    }
    // ... same for limit, resource
}
```

### 4.3 Pre-Flight Check

Before every API call, the rate limiter performs a pre-flight check:

```
fn check_before_call(&self) -> Result<(), RateLimitError> {
    // 1. Check remaining primary limit
    let remaining = self.remaining.load();
    let reset_at = self.reset_at.load();

    if remaining <= self.pause_threshold {
        let wait_secs = reset_at - current_unix_time();
        if wait_secs > 0 {
            return Err(RateLimitError::PrimaryLimitExhausted {
                reset_in: Duration::from_secs(wait_secs),
            });
        }
    }

    // 2. Check concurrent request cap
    let concurrent = self.concurrent_requests.load();
    if concurrent >= self.max_concurrent {
        return Err(RateLimitError::TooManyConcurrent);
    }

    // 3. Warn if approaching limit
    if remaining <= self.warn_threshold {
        log::warn!("Rate limit low: {}/{} remaining, resets in {}s",
            remaining, self.limit.load(), reset_at - current_unix_time());
    }

    Ok(())
}
```

### 4.4 Retry Strategy

When a request fails with `429 Too Many Requests` or a rate limit exhaustion:

```
async fn execute_with_retry<F, T>(
    &self,
    request_fn: F,
    max_retries: u32,
) -> Result<T>
where F: Fn() -> Future<Output = Result<T>> + Clone
{
    let mut retries = 0;
    let mut backoff = self.base_backoff();

    loop {
        // Pre-flight check
        self.check_before_call()?;

        // Track concurrent
        self.concurrent_requests.fetch_add(1);

        // Execute
        let result = request_fn().await;
        self.concurrent_requests.fetch_sub(1);

        match result {
            Ok(val) => return Ok(val),

            Err(e) if e.is_rate_limit_error() => {
                retries += 1;
                if retries > max_retries {
                    return Err(e);
                }

                // Read retry-after header if present (secondary rate limit)
                // Or compute from x-ratelimit-reset (primary rate limit)
                let wait = e.retry_after()
                    .unwrap_or(backoff);

                log::warn!("Rate limited. Retry {}/{} in {}s",
                    retries, max_retries, wait.as_secs());

                // Emit event to UI (show "Rate limited, retrying in N seconds...")
                self.progress_tx.send(AppEvent::RateLimited { retry_in: wait })?;

                tokio::time::sleep(wait).await;
                backoff = backoff * 2;  // exponential for successive failures
            }

            Err(e) => return Err(e),  // non-rate errors: pass through immediately
        }
    }
}
```

**Base backoff:** Starts at 1 second. Doubles on each retry (1s, 2s, 4s, 8s). Capped at 60 seconds.

**Retry-after priority:**
1. `Retry-After` response header (seconds) — for secondary rate limits
2. `x-ratelimit-reset` header (epoch seconds) — for primary rate limit exhaustion
3. Exponential backoff — fallback when no headers

### 4.5 Concurrency Model

The sync engine (which performs git clone/fetch) is separate from the GitHub integration (which performs API calls). API calls are:
- Authentication: 1-3 sequential calls, low urgency
- Discovery: Paginated, sequential-by-page but parallel-across-orgs
- Token validation: 1 call, high urgency

**Discovery parallelism for "All Accessible":**

```
// Phase 1: List orgs (1 API call)
let orgs = github_api.list_my_orgs().await?;

// Phase 2: User repos + org repos in parallel
// - 1 call for user repos (paginated)
// - 1 call per org (paginated)
//
// Semaphore with max 4 permits — respects secondary rate limit (100 concurrent)
let semaphore = Arc::new(Semaphore::new(4));

// Spawn tasks
let mut handles = vec![];
handles.push(spawn_paginated_fetch(semaphore.clone(), github_api.list_my_repos()));
for org in &orgs {
    handles.push(spawn_paginated_fetch(semaphore.clone(), github_api.list_org_repos(&org.login)));
}

// Collect all results
// Pages within each fetch are sequential (pagination); fetches across orgs are parallel
let all_repos = futures::future::join_all(handles).await;
```

**This means:** For a user in 10 orgs, we make at most 4 concurrent paginated fetches. Each fetch streams pages one at a time. Total concurrent API calls ≤ 4 (+1 for rate limit pre-flight). Well within the 100 concurrent secondary limit.

### 4.6 Rate Limit UI

The rate limit state is surfaced in the UI:

**Title bar (when rate limit is low):**
```
│ ▓▓ git-vacuum ▓▓   user: prakash   API: 2,847/5,000 (resets 43m)   ⚠ 47 repos │
```

**During discovery (progress bar annotation):**
```
│   Rate limit: ████████████████░░░░  2,847 remaining  (57%)                      │
```

**Sync Center (when paused for rate limit):**
```
│   ⏸ PAUSED — GitHub rate limit. Resuming in 3:42...                              │
```

---

## 5. Data Synchronization Strategy

### 5.1 The Merge Problem

After fetching remote repos from the GitHub API, we have a `Vec<RemoteRepo>`. We also have existing rows in the `repositories` SQLite table (from a previous discovery run). The challenge: **merge remote data with cached local state without losing information.**

Specifically:
- Remote data overwrites cache: description, stars, pushed_at, topics, visibility, archived status
- Remote data must NOT overwrite local state: clone_status, local_path, selected, behind_count, last_synced_at
- Repos that exist in cache but NOT in the remote response may have been deleted on GitHub
- Repos that exist in both may have changed owner (transferred between orgs)

### 5.2 Merge Algorithm

```
fn merge_remote_with_local(
    remote_repos: Vec<RemoteRepo>,
    cached_repos: Vec<RepoRow>,       // from SQLite
    source_scope: DiscoveryScope,     // which owner's repos we fetched
) -> MergeResult {
    // Index cached repos by github_id for O(1) lookup
    let cached_by_id: HashMap<i64, RepoRow> = cached_repos
        .into_iter()
        .map(|r| (r.github_id, r))
        .collect();

    let mut merged = Vec::new();
    let mut deleted_ids = Vec::new();

    for remote in &remote_repos {
        if let Some(cached) = cached_by_id.get(&remote.github_id) {
            // REPO EXISTS IN BOTH: merge
            merged.push(MergedRepo {
                // Remote data (overwrites cache)
                github_id: remote.github_id,
                owner_login: remote.owner_login.clone(),    // owner may have changed
                name: remote.name.clone(),
                full_name: remote.full_name.clone(),
                description: remote.description.clone(),
                language: remote.language.clone(),
                default_branch: remote.default_branch.clone(),
                visibility: remote.visibility,
                is_fork: remote.is_fork,
                is_archived: remote.is_archived,
                size_kb: remote.size_kb,
                stars: remote.stars,
                open_issues: remote.open_issues,
                license_spdx: remote.license_spdx.clone(),
                topics_json: serde_json::to_string(&remote.topics),
                clone_url_ssh: remote.clone_url_ssh.clone(),
                clone_url_https: remote.clone_url_https.clone(),
                pushed_at: remote.pushed_at,
                updated_at_gh: Some(remote.updated_at),
                discovered_at: now(),
                deleted_on_remote: false,

                // Preserved local state (NOT overwritten by remote)
                selected: cached.selected,
                // clone_status, local_path, etc. live in local_clones table (not overwritten)
            });
        } else {
            // NEW REPO: insert fresh
            merged.push(MergedRepo {
                // ... all remote fields ...
                deleted_on_remote: false,
                selected: true,  // default: select new repos
                discovered_at: now(),
            });
        }
    }

    // Mark repos in cache but NOT in remote as deleted
    // BUT only if they belong to the same scope we just fetched
    let remote_ids: HashSet<i64> = remote_repos.iter().map(|r| r.github_id).collect();
    for (id, cached) in &cached_by_id {
        if !remote_ids.contains(id) && is_in_scope(cached, &source_scope) {
            deleted_ids.push(*id);
        }
    }

    MergeResult { merged, deleted_ids }
}
```

### 5.3 Scope-Aware Deletion Marking

When we discover repos for a specific scope (e.g., "My Repos" only), we must NOT mark repos from other scopes as deleted. For example:

- User discovers "My Repos" → only repos owned by the user are fetched
- A repo from "acme-corp" org exists in the cache
- This repo was NOT in the "My Repos" response (correctly — it's an org repo)
- We must NOT mark it as `deleted_on_remote`

**Scope resolution:**

```
enum DiscoveryScope {
    MyRepos,                // GET /user/repos → only user's personal repos
    Org(String),            // GET /orgs/{org}/repos → only that org's repos
    Starred,                // GET /user/starred → any owner (don't prune for this scope)
    All,                    // My repos + all orgs → safe to prune everything
}

fn is_in_scope(repo: &RepoRow, scope: &DiscoveryScope) -> bool {
    match scope {
        DiscoveryScope::MyRepos => repo.owner_account_id.is_some() && repo.owner_org_id.is_none(),
        DiscoveryScope::Org(login) => repo.owner_login == *login,
        DiscoveryScope::Starred => false,  // never prune from starred discovery
        DiscoveryScope::All => true,       // full discovery: prune anything not found
    }
}
```

### 5.4 Discovery Caching (Tiered Freshness)

To minimize API calls, we implement a cache freshness policy:

| Data | Freshness | Refresh Trigger |
|------|-----------|----------------|
| Repository metadata (from API) | Cached in SQLite `repositories` table. Stale after 24 hours. | User triggers "Refresh" in Explorer. Auto-refresh on startup if cache is >24h old. |
| Repository local state (clone_status, behind_count) | Updated by sync engine and stats refresh. Not tied to API cache. | After sync completes. After `RefreshDashboardStats` effect runs. |
| Organization list | Cached in `github_orgs` table. Stale after 7 days. | Refreshed during "All Accessible" discovery or when user enters an org name in Explorer. |
| User profile | Cached in `github_accounts`. Stale after 24 hours. | Refreshed on startup (token validation). |
| Topics (normalized) | Populated during discovery. Not independently refreshed. | On every discovery run (topics come from the same API response). |

**Stale cache handling:**
- When the user opens Explorer and the cache is older than 24 hours, the API data is still shown (instant load from SQLite). A "Data may be stale. Press r to refresh." banner appears.
- The user can work with cached data immediately — viewing, filtering, selecting repos.
- The refresh is opt-in (press `r`) or automatic on first launch after auth.

### 5.5 Incremental Discovery (Future Optimization)

For v1.0, we always do full discovery. For a future optimization:

**Incremental discovery:** Use `GET /user/repos?since=<repo_id>` or `GET /repositories?since=<repo_id>` (the global "list all public repos" endpoint, sorted by ID) to fetch only repos created after the last discovery. This reduces 500 API calls to 5-10 for daily refresh.

This requires storing `max(github_id)` from the last discovery and using it as the `since` parameter. Only works for repos ordered by ID — not all GitHub endpoints support this.

---

## 6. Error Handling

### 6.1 Error Type Hierarchy

```
enum GithubError {
    // ── Authentication ──
    Auth(AuthError),

    // ── Rate Limiting ──
    RateLimit(RateLimitError),

    // ── Network ──
    Network(NetworkError),

    // ── API responses ──
    Api(ApiError),

    // ── Data parsing ──
    Parse(ParseError),

    // ── Configuration ──
    Config(ConfigError),
}

enum AuthError {
    InvalidToken,             // 401: token doesn't authenticate
    ExpiredToken,             // token has expired
    InsufficientScopes {      // 403: token lacks required scopes
        required: Vec<String>,
        actual: Vec<String>,
    },
    TokenRevoked,             // token was explicitly revoked
    AccountSuspended,         // 403: user account is suspended/banned
    SsoRequired {             // 403: org requires SAML SSO authorization
        org: String,
    },
    DeviceFlowDisabled,       // device flow not enabled for this OAuth app
}

enum RateLimitError {
    PrimaryLimitExhausted {
        reset_in: Duration,
    },
    SecondaryLimitHit {
        retry_after: Duration,
    },
    AbuseDetectionMechanism {
        retry_after: Duration,
        message: String,
    },
}

enum NetworkError {
    Timeout,                  // request exceeded timeout
    ConnectionRefused,        // cannot reach api.github.com
    DnsResolutionFailed,      // DNS lookup failed
    TlsError(String),         // TLS certificate/handshake error
    TooManyRedirects,         // redirect loop
}

enum ApiError {
    NotFound {                // 404
        resource: String,
    },
    Forbidden {               // 403 (non-auth)
        message: String,
    },
    ServerError {             // 5xx
        status: u16,
        message: String,
    },
    UnprocessableEntity {     // 422
        errors: Vec<String>,
    },
    UnexpectedStatus {        // any other status
        status: u16,
        body: String,
    },
}

enum ParseError {
    JsonDeserialization {
        context: String,
        source: serde_json::Error,
    },
    InvalidHeader {
        header: String,
        value: String,
    },
    MissingField {
        field: String,
        context: String,
    },
}

enum ConfigError {
    MissingClientId,
    InvalidBaseUrl(String),
}
```

### 6.2 Error Mapping Strategy

Errors from Octocrab's `Error` type are mapped to our `GithubError` at the boundary of the `GithubApi` trait implementation. The mapping is in `error.rs`:

```
fn map_octocrab_error(err: octocrab::Error) -> GithubError {
    match err {
        // Octocrab wraps GitHub's error responses
        octocrab::Error::GitHub { source, .. } => {
            match source.status_code {
                401 => GithubError::Auth(AuthError::InvalidToken),
                403 if is_rate_limited(&source) =>
                    GithubError::RateLimit(parse_rate_limit_error(&source)),
                403 if is_sso_required(&source) =>
                    GithubError::Auth(AuthError::SsoRequired { org: parse_org(&source) }),
                403 =>
                    GithubError::Api(ApiError::Forbidden { message: source.message }),
                404 =>
                    GithubError::Api(ApiError::NotFound { resource: source.documentation_url }),
                422 =>
                    GithubError::Api(ApiError::UnprocessableEntity { errors: source.errors }),
                500..=599 =>
                    GithubError::Api(ApiError::ServerError {
                        status: source.status_code,
                        message: source.message,
                    }),
                _ =>
                    GithubError::Api(ApiError::UnexpectedStatus {
                        status: source.status_code,
                        body: source.message,
                    }),
            }
        }
        // HTTP/network errors from the underlying reqwest client
        octocrab::Error::Http { source, .. } => {
            if source.is_timeout() {
                GithubError::Network(NetworkError::Timeout)
            } else if source.is_connect() {
                GithubError::Network(NetworkError::ConnectionRefused)
            } else {
                GithubError::Network(NetworkError::TlsError(source.to_string()))
            }
        }
        // All other Octocrab errors
        other => GithubError::from(other),
    }
}
```

### 6.3 User-Facing Error Messages

The service layer converts `GithubError` to user-friendly messages before emitting `AppEvent`:

| GithubError | User Message | UI Action |
|-------------|-------------|-----------|
| `Auth(InvalidToken)` | "Authentication failed. Your token may be expired or revoked." | Show auth gate |
| `Auth(InsufficientScopes)` | "Your token is missing required permissions: repo, read:org. Create a new token with these scopes." | Show scopes diff in auth gate |
| `Auth(SsoRequired { org })` | "Organization 'acme-corp' requires SAML SSO. Authorize your token in GitHub settings." | Show SSO authorization link |
| `RateLimit(PrimaryLimitExhausted { reset_in })` | "GitHub API rate limit reached. Retrying in Xm Ys." | Pause, show countdown |
| `RateLimit(SecondaryLimitHit { retry_after })` | "GitHub rate limit. Cooling down for Xs." | Pause, show countdown |
| `Network(Timeout)` | "Request timed out. Check your internet connection." | Show retry button |
| `Network(ConnectionRefused)` | "Cannot reach api.github.com. Check your network." | Show offline banner |
| `Api(NotFound { resource })` | "The requested resource was not found on GitHub." | Log only (during discovery: skip repo) |
| `Api(ServerError { status })` | "GitHub returned an error (status {status}). Retrying..." | Retry with backoff |

### 6.4 Retry Policy

| Error Category | Retry? | Max Retries | Backoff |
|---------------|--------|-------------|---------|
| Auth errors | No | 0 | N/A — requires user action |
| Primary rate limit | Yes | Wait for reset, then 0 retries | Wait for `x-ratelimit-reset` |
| Secondary rate limit | Yes | 3 | Retry-After header or exponential (1s, 2s, 4s) |
| Network timeout | Yes | 3 | Exponential (1s, 2s, 4s) |
| Network connection refused | Yes | 3 | Exponential (2s, 4s, 8s) |
| 500 Server Error | Yes | 3 | Exponential (1s, 2s, 4s) |
| 404 Not Found | No | 0 | Log and skip (repo was deleted) |
| 403 Forbidden (non-auth) | No | 0 | Log and skip (access denied to specific resource) |
| 422 Unprocessable | No | 0 | Log and skip |

---

## 7. Discovery Progress & Cancellation

### 7.1 Progress Events During Discovery

Discovery is the most visible long-running GitHub API operation. The user clicks "Refresh" in Explorer and waits. We must provide feedback.

**Progress event stream:**

```
// Emitted by the discovery service while fetching pages
enum DiscoveryEvent {
    Started {
        source: RepoSource,
        estimated_total: Option<usize>,  // from first page's Link header (last page number × 100)
    },
    PageFetched {
        page_num: usize,
        repos_in_page: usize,
        total_so_far: usize,
    },
    RateLimitPaused {
        remaining: u32,
        reset_in: Duration,
    },
    RateLimitResumed,
    MergingWithCache,    // API fetch done, now merging with SQLite
    SavingToDatabase,    // Writing merged data to SQLite
    Completed {
        total_repos: usize,
        new_repos: usize,
        deleted_repos: usize,
        duration: Duration,
    },
    Failed {
        error: String,
        repos_so_far: usize,  // partial data available
    },
}
```

**These map to AppEvent variants** that the reducer uses to update `ExplorerTabState.loading` and show a progress indicator.

### 7.2 Cancellation

The user can cancel discovery at any time by pressing `Esc` or navigating away from Explorer.

**Cancellation mechanism:** The `cancel_rx` watch channel is checked at the start of each page fetch and after each page is parsed. If cancelled:
1. The `PagedStream` stops fetching new pages.
2. Already-fetched repos are saved to SQLite (partial discovery is better than no discovery).
3. `AppEvent::DiscoveryFailed` is NOT emitted — partial success is a success, just incomplete.
4. The Explorer shows a "Discovery cancelled. 237 of ~500 repos fetched." banner.

---

## 8. Organization Support

### 8.1 Org Discovery

Organizations are discovered alongside repositories. The flow:

```
GET /user/orgs
  → Returns all orgs the authenticated user belongs to
  → Each org has: id, login, description, avatar_url
  → Upsert to github_orgs table
  → Upsert org_memberships rows (many-to-many for future multi-account)
```

### 8.2 Org Repo Discovery

For each org the user belongs to:

```
GET /orgs/{org}/repos?type=all&sort=updated&per_page=100
  → type=all includes public, private, and internal repos
  → Requires token with read:org scope or user to be an org member
  → Paginated via PagedStream
  → Each repo's owner_org_id is set to the org's id
```

### 8.3 SAML SSO

Some organizations require SAML Single Sign-On for token access. When a token hasn't been authorized for SSO:

- API returns `403` with `X-GitHub-SSO` header containing the org's SSO URL
- We map this to `AuthError::SsoRequired { org }`
- The UI shows: "Your token needs SSO authorization for acme-corp. Visit: https://github.com/orgs/acme-corp/sso"
- The user can still access other orgs' repos — only the SSO org's repos fail

### 8.4 Private Repository Access

Private repos are included in discovery automatically when the token has `repo` scope and the user has access. No additional configuration needed. The visibility field in `RemoteRepo` tracks whether a repo is `public`, `private`, or `internal`.

**Internal repos** (GitHub Enterprise feature): Treated identically to private repos in MVP. The visibility field distinguishes them in the Explorer UI.

---

## 9. GitHub Enterprise Support (Design Forward)

While MVP targets `github.com`, the architecture supports GitHub Enterprise from day one:

- `OctocrabBuilder::base_uri()` accepts any URL
- The `base_url` is configurable via settings: `github.base_url = "https://github.mycompany.com/api/v3"`
- API endpoints are identical between cloud and enterprise
- Rate limits may differ (Enterprise instances often have higher or no limits)
- The `client.rs` builder reads the base URL from settings

This requires zero code changes to the GitHub integration layer — only configuration.

---

## 10. Summary: API Budget

For a typical user with 200 personal repos across 5 orgs (200 more org repos = 400 total):

| Operation | API Calls | Rate Limit Cost |
|-----------|-----------|----------------|
| Startup: validate token | 1 | 1 |
| Discovery: list orgs | 1 | 1 |
| Discovery: user repos (200 repos ÷ 100/page) | 2 | 2 |
| Discovery: org repos (200 repos ÷ 100/page) | 2 | 2 |
| Discovery: merge + save to SQLite | 0 | 0 |
| **Total per discovery** | **6** | **6** |

Against a 5,000/hour budget, one discovery costs **0.12%** of the rate limit. Even refreshing every 5 minutes, the user would use only 72 calls/hour — well within limits.

Worst case (1,000+ repos across 20 orgs):

| Operation | API Calls | Rate Limit Cost |
|-----------|-----------|----------------|
| Discovery: list orgs | 1 | 1 |
| Discovery: user repos | 10 | 10 |
| Discovery: 20 orgs × avg 5 pages each | 100 | 100 |
| **Total worst case** | **111** | **111** |

**111 calls = 2.2% of 5,000/hour budget.** Even power users are safe.

The only risk is secondary rate limits (concurrency) — which we handle with the semaphore-limited parallel fetcher (max 4 concurrent org fetches).
