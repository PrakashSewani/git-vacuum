# Git-Vacuum — Clone & Synchronization Engine

**Module:** `service/sync_engine.rs` (orchestrator) + `service/` sub-modules  
**Infrastructure dependency:** `git/` module (via `GitOps` trait)  
**Concurrency:** Multi-stage Tokio task-based pipeline with semaphore backpressure  
**Output:** Stream of `AppEvent::Sync*` events via `progress_tx` + final summary via `app_tx`

---

## 1. Architecture Overview

The sync engine is a **four-stage async pipeline** orchestrated by a single coordinator task running on `tokio::spawn`. Each stage feeds into the next via channels. The pipeline is designed so that slow stages don't block fast stages, and progress events flow to the UI at 60fps regardless of git operation duration.

```
                         ┌──────────┐
     SyncPlan            │ COORDINATOR│
  (Vec<JobSpec>) ──────▶ │  TASK      │
                         └─────┬─────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
        ┌──────────┐   ┌──────────────┐  ┌──────────────┐
        │ STAGE 1  │   │   PROGRESS   │  │  PERSISTENCE │
        │ QUEUE +  │   │   TRACKER    │  │    WRITER    │
        │ DISPATCH │   │              │  │              │
        └────┬─────┘   └──────┬───────┘  └──────┬───────┘
             │                │                  │
             ▼                │                  │
    ┌────────────────┐       │                  │
    │ STAGE 2        │       │                  │
    │ WORKER POOL    │── progress events ───────┤
    │ (N concurrent) │       │                  │
    │                │── completion events ─────┤
    └───────┬────────┘       │                  │
            │                │                  │
            ▼                ▼                  ▼
    ┌──────────────────────────────────────────────┐
    │ STAGE 3: RESULT COLLECTOR + RETRY DECIDER    │
    └──────────────────────┬───────────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ STAGE 4      │
                    │ SUMMARY      │
                    │ + CLEANUP    │
                    └──────────────┘
```

**Stage responsibilities:**

| Stage | Name | Role | Concurrency |
|-------|------|------|-------------|
| 1 | Queue + Dispatch | Accepts the sync plan. Determines what already exists on disk. Enqueues jobs. Feeds the worker pool. | Single task |
| 2 | Worker Pool | Executes git clone/fetch operations. Emits per-job progress and completion events. | N concurrent (semaphore) |
| 3 | Result Collector | Aggregates completion events. Decides retries. Tracks overall progress. | Single task |
| 4 | Summary + Cleanup | Persists sync run to SQLite. Emits final AppEvent::SyncAllCompleted. | Single task |

**Why a staged pipeline instead of a simple `join_all`?**

A simple `join_all(spawn(...))` over all repos fails to handle:
- **Backpressure:** 500 concurrent clones would saturate disk I/O. The semaphore limits active operations to `N`.
- **Pause/resume:** With `join_all`, there's no way to inject a pause signal between jobs.
- **Progress streaming:** With `join_all`, the coordinator waits for ALL futures to complete before emitting a summary. The UI sees a progress bar that jumps from 0% to 100%. The staged pipeline emits per-job progress continuously.
- **Failure recovery:** With `join_all`, a single failing job doesn't prevent others from starting. But there's no centralized retry logic — each job would need its own retry loop, complicating global decisions like "stop retrying if we're out of disk space."

---

## 2. Core Data Structures

### 2.1 SyncPlan

The input to the sync engine. Constructed by the application layer from selected repos + user options.

```
struct SyncPlan {
    run_id: i64,                       // pre-allocated in SQLite (sync_runs row)
    jobs: Vec<JobSpec>,                // ordered list of repos to process
    base_path: PathBuf,                // e.g., ~/git-vacuum
    concurrency: usize,               // max concurrent git operations (1-50)
    protocol: CloneProtocol,           // SSH or HTTPS
    options: SyncOptions,              // mirror, prune, include_wikis, lfs, timeout
}

enum CloneProtocol {
    Ssh,
    Https { token: String },           // token needed for HTTPS clone of private repos
}

struct SyncOptions {
    timeout_per_job: Duration,         // max time for a single clone/fetch (default: 30 min)
    mirror: bool,                       // use --mirror clone
    include_wikis: bool,                // also clone <repo>.wiki.git
    fetch_lfs: bool,                    // run git lfs fetch --all after clone/fetch
    prune_deleted: bool,               // delete local repos not in the sync plan
    retry_failed: bool,                // whether to retry failed jobs
    max_retries: u32,                  // max retries per job (default: 2)
    retry_delay_base: Duration,        // base delay between retries (default: 5s)
}
```

### 2.2 JobSpec

A single unit of work. One repo = one job. If `include_wikis` is true and the wiki exists, the wiki is a separate job (cloned to `<repo>.wiki` path).

```
struct JobSpec {
    job_id: JobId,                     // unique within this sync run
    repo_full_name: String,            // "acme/web-frontend"
    repo_github_id: i64,
    owner_login: String,
    clone_url: String,                 // resolved: SSH or HTTPS URL
    local_path: PathBuf,               // ~/git-vacuum/acme/web-frontend
    operation: PlannedOperation,       // what to do
    priority: Priority,                // control dispatch order
}

enum PlannedOperation {
    Clone,                             // repo doesn't exist locally → full clone
    Sync,                              // repo exists → fetch + fast-forward
    Mirror,                            // bare mirror clone (--mirror flag)
    Skip { reason: SkipReason },       // no-op (already up-to-date snapshot, or excluded)
}

enum SkipReason {
    AlreadyUpToDate,                   // local clone exists and behind_count == 0
    LocalOnly,                         // repo deleted on remote, no sync needed
    NoAccess,                          // token lacks access to this repo
}

enum Priority {
    High = 0,    // small repos first (fast feedback)
    Normal = 1,
    Low = 2,     // large repos, archived repos
}
```

### 2.3 JobQueue

A priority-aware dispatch queue that feeds the worker pool. Not a FIFO — it prioritizes small/fast jobs to give the user rapid visible progress.

```
struct JobQueue {
    pending: VecDeque<JobSpec>,         // ordered by priority, then size_kb ascending
    active: HashMap<JobId, RunningJob>, // jobs currently being processed
    completed: HashMap<JobId, JobOutcome>, // finished jobs (success or terminal failure)
    retry_queue: VecDeque<RetryEntry>,  // jobs waiting to be retried
    pause_flag: Arc<AtomicBool>,        // shared: workers check before taking next job
    cancel_flag: watch::Receiver<bool>, // shared: workers abort active operations
    enqueued_at: Instant,               // when the sync started
}

struct RunningJob {
    spec: JobSpec,
    started_at: Instant,
    attempt: u32,                       // 0 = first attempt, 1+ = retry
    progress: JobProgress,
}

struct RetryEntry {
    spec: JobSpec,
    attempt: u32,
    delay_until: Instant,              // when this retry is eligible
}
```

### 2.4 JobOutcome

The result of a single job after completion (including retries):

```
enum JobOutcome {
    Success {
        spec: JobSpec,
        result: OperationResult,
        duration: Duration,
        attempts: u32,
    },
    Failed {
        spec: JobSpec,
        error: SyncError,
        attempts: u32,
        retries_exhausted: bool,
    },
    Skipped {
        spec: JobSpec,
        reason: SkipReason,
    },
    Cancelled {
        spec: JobSpec,
        partial_result: Option<OperationResult>, // partial clone data if cancelled mid-op
    },
}

struct OperationResult {
    operation: CompletedOperation,
    bytes_transferred: u64,
    new_commits: u32,
    size_on_disk_kb: u64,
}

enum CompletedOperation {
    Cloned,
    Synced,
    UpToDate,        // fetch returned 0 new commits
    MirrorCloned,
}
```

### 2.5 Progress Tracker

Aggregates per-job progress into overall progress metrics for the UI.

```
struct ProgressTracker {
    total_jobs: usize,
    completed: AtomicU32,
    succeeded: AtomicU32,
    failed: AtomicU32,
    skipped: AtomicU32,

    bytes_transferred: AtomicU64,
    bytes_total_estimate: AtomicU64,    // sum of repo sizes from API (approximate)

    // Rolling throughput calculation
    throughput_window: Mutex<VecDeque<(Instant, u64)>>,  // (timestamp, bytes_sample)

    // Per-job progress (live)
    active_jobs: Mutex<Vec<ActiveJobProgress>>,

    started_at: Instant,
}

struct ActiveJobProgress {
    job_id: JobId,
    repo_full_name: String,
    operation: PlannedOperation,
    phase: JobPhase,
    bytes_done: u64,
    bytes_total: u64,
    percent: f32,                       // 0.0 - 1.0
    update_count: u64,                  // incremented on each progress update → UI re-render
}

enum JobPhase {
    Queued,
    Connecting,                         // resolving DNS / SSH handshake
    Receiving,                          // fetching objects
    Resolving,                          // resolving deltas
    CheckingOut,                        // writing working tree
    Verifying,                          // fsck / LFS fetch
}
```

---

## 3. Job Lifecycle State Machine

Every job transitions through these states. The state machine is explicit — no implicit state encoded in `Option<T>` checks scattered across the codebase.

```
                        ┌─────────┐
                        │ QUEUED  │
                        └────┬────┘
                             │
                    ┌────────┴────────┐
                    │ Should skip?    │
                    │ (SkipReason)    │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │ skip         │ process      │ dispatched
              ▼              │              ▼
        ┌──────────┐        │       ┌────────────┐
        │ SKIPPED  │        │       │  RUNNING   │
        └──────────┘        │       └──────┬─────┘
                             │              │
                             │     ┌────────┼────────┐
                             │     │        │        │
                             │  complete   fail   cancelled
                             │     │        │        │
                             │     ▼        ▼        ▼
                             │ ┌────────┐ ┌──────┐ ┌───────────┐
                             │ │SUCCESS │ │FAILED│ │ CANCELLED │
                             │ └────────┘ └──┬───┘ └───────────┘
                             │               │
                             │      ┌────────┴────────┐
                             │      │ Retriable?      │
                             │      │ + attempts < max│
                             │      └────────┬────────┘
                             │               │
                             │    ┌──────────┼──────────┐
                             │    │ yes      │ no        │
                             │    ▼          ▼           │
                             │ ┌────────┐ ┌──────────────┐
                             │ │RETRYING│ │TERMINAL_FAIL │
                             │ └───┬────┘ └──────────────┘
                             │     │
                             │     │ backoff expired
                             │     │
                             │     └──────▶ QUEUED (attempt + 1)
                             │
                             └───────────────────────────
```

**State invariants:**
1. A job can be in exactly one state at any time.
2. QUEUED → RUNNING transition requires a semaphore permit (concurrency gate).
3. FAILED → RETRYING only transitions if `attempts < max_retries` AND the error is retriable (see §5).
4. RUNNING → CANCELLED happens when the cancel signal fires mid-operation. The git operation is terminated (kill signal to the git process, or git2-rs remote stop).
5. RETRYING → QUEUED happens when the backoff delay expires. The job re-enters the queue at the front (to retry promptly) or back (to not starve other jobs).

---

## 4. Parallel Cloning — Worker Pool Design

### 4.1 Architecture

The worker pool is not a standalone component. It's the union of three cooperating mechanisms:

1. **Tokio Semaphore** — limits concurrent git operations to `concurrency` (default: 8).
2. **Blocking Thread Pool** (`tokio::task::spawn_blocking`) — git2-rs operations are CPU-bound and blocking. They must run on the blocking pool, not on async worker threads.
3. **JobQueue** — feeds the semaphore. When a worker finishes, it signals completion. The dispatcher then dequeues the next job and spawns a new worker.

### 4.2 Dispatcher Loop

The dispatcher is a single Tokio task that runs for the duration of the sync:

```
async fn dispatcher(
    mut queue: JobQueue,
    semaphore: Arc<Semaphore>,
    git: Arc<dyn GitOps>,
    progress_tracker: Arc<ProgressTracker>,
    options: SyncOptions,
    result_tx: mpsc::Sender<JobOutcome>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    cancel_rx: watch::Receiver<bool>,
) {
    loop {
        // 1. Check if we should stop
        if *cancel_rx.borrow() {
            // Drain remaining running jobs will complete; no new jobs start
            break;
        }

        // 2. Check pause flag
        if queue.pause_flag.load(Ordering::Acquire) {
            // Emit paused event (once)
            let _ = event_tx.send(AppEvent::SyncPaused);
            // Wait for resume or cancel
            tokio::select! {
                _ = wait_for_resume(&queue.pause_flag) => {}
                _ = cancel_rx.changed() => break,
            }
            let _ = event_tx.send(AppEvent::SyncResumed);
            continue;
        }

        // 3. Check retry queue first (retries have priority)
        let now = Instant::now();
        if let Some(entry) = queue.pop_eligible_retry(now) {
            queue.enqueue_front(entry.spec, entry.attempt);
        }

        // 4. Dequeue next job
        let Some(job) = queue.dequeue() else {
            // No pending jobs. Check if all workers are done.
            if queue.active.is_empty() && queue.retry_queue.is_empty() {
                break; // All done
            }
            // Active workers still running. Wait a tick.
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        };

        // 5. Acquire semaphore permit (blocks if at concurrency limit)
        let permit = match tokio::time::timeout(
            Duration::from_secs(1), // prevent hanging on semaphore during shutdown
            semaphore.clone().acquire_owned()
        ).await {
            Ok(Ok(permit)) => permit,
            _ => continue, // timeout or closed → check cancel flag and retry
        };

        // 6. Spawn worker
        let job_id = job.job_id;
        queue.mark_active(job_id, RunningJob { spec: job.clone(), started_at: Instant::now(), attempt: job.attempt, progress: JobProgress::default() });

        let git = git.clone();
        let event_tx = event_tx.clone();
        let result_tx = result_tx.clone();
        let cancel_rx = cancel_rx.clone();
        let tracker = progress_tracker.clone();

        tokio::spawn(async move {
            let outcome = execute_job(job, git, &tracker, &event_tx, &cancel_rx).await;
            drop(permit); // release semaphore
            let _ = result_tx.send(outcome).await;
        });
    }

    // Wait for active workers to drain
    while !queue.active.is_empty() {
        // Drain result_tx — workers send outcomes here
        // This is handled by the collector task (see below)
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

### 4.3 Job Execution

Each worker runs `execute_job` which encapsulates the full lifecycle of a single operation:

```
async fn execute_job(
    job: JobSpec,
    git: Arc<dyn GitOps>,
    tracker: &ProgressTracker,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
    cancel_rx: &watch::Receiver<bool>,
) -> JobOutcome {
    // ── 1. Pre-flight check ──
    if *cancel_rx.borrow() {
        return JobOutcome::Cancelled { spec: job, partial_result: None };
    }

    // ── 2. Ensure parent directory exists ──
    if let Some(parent) = job.local_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return JobOutcome::Failed {
                spec: job,
                error: SyncError::Filesystem(format!("Cannot create directory: {}", e)),
                attempts: 1,
                retries_exhausted: false,
            };
        }
    }

    // ── 3. Execute clone or sync ──
    let result = match job.operation {
        PlannedOperation::Clone => {
            // Emit start event
            let _ = event_tx.send(AppEvent::SyncCloneStarted {
                repo_full_name: job.repo_full_name.clone(),
                job_id: job.job_id,
            });

            // Track as active
            tracker.register_active(job.job_id, &job);

            // Execute clone on blocking thread pool
            let git = git.clone();
            let cancel_rx = cancel_rx.clone();

            let clone_result = tokio::task::spawn_blocking(move || {
                git.clone_with_progress(
                    &job.clone_url,
                    &job.local_path,
                    move |progress| {
                        // Progress callback — called from libgit2 C code
                        // Send progress via a oneshot-ish mechanism
                        // (see §7 for progress plumbing)
                    },
                    cancel_rx,  // pass cancellation signal into git2-rs
                )
            }).await;

            match clone_result {
                Ok(Ok(stats)) => {
                    // Check if cancelled during clone
                    if stats.cancelled {
                        JobOutcome::Cancelled {
                            spec: job,
                            partial_result: Some(OperationResult {
                                operation: CompletedOperation::Cloned,
                                bytes_transferred: stats.received_bytes,
                                new_commits: 0,
                                size_on_disk_kb: 0,
                            }),
                        }
                    } else {
                        let _ = event_tx.send(AppEvent::SyncCloneCompleted {
                            repo_full_name: job.repo_full_name.clone(),
                            job_id: job.job_id,
                            size_bytes: stats.received_bytes,
                            duration: stats.duration,
                        });
                        tracker.mark_completed(job.job_id, true);
                        JobOutcome::Success {
                            spec: job,
                            result: OperationResult {
                                operation: CompletedOperation::Cloned,
                                bytes_transferred: stats.received_bytes,
                                new_commits: 0,
                                size_on_disk_kb: stats.received_bytes / 1024,
                            },
                            duration: stats.duration,
                            attempts: 1,
                        }
                    }
                }
                Ok(Err(e)) => JobOutcome::Failed {
                    spec: job,
                    error: map_git_error(e),
                    attempts: 1,
                    retries_exhausted: false,
                },
                Err(join_err) => JobOutcome::Failed {
                    spec: job,
                    error: SyncError::Internal(join_err.to_string()),
                    attempts: 1,
                    retries_exhausted: false,
                },
            }
        }

        PlannedOperation::Sync => {
            // Similar structure — emit SyncFetchStarted, call git.fetch() via
            // spawn_blocking, then git.status() to check behind_count, then
            // git fast-forward if needed. Emit SyncFetchCompleted.
            // ...
            JobOutcome::Success { /* ... */ }
        }

        PlannedOperation::Mirror => {
            // Mirror clone — similar to clone but with --mirror semantics
            // ...
            JobOutcome::Success { /* ... */ }
        }

        PlannedOperation::Skip { reason } => {
            JobOutcome::Skipped { spec: job, reason }
        }
    };

    result
}
```

### 4.4 Concurrency Backpressure

The `tokio::sync::Semaphore` with `concurrency` permits is the sole backpressure mechanism. Workers acquire a permit before starting (step 5 in the dispatcher). They release it when done (via `drop(permit)`).

**Why a semaphore and not a bounded channel?**
- A bounded channel enforces backpressure at the job submission level — the dispatcher blocks when trying to send a job to a full channel. This couples submission and execution.
- A semaphore decouples them. The dispatcher dequeues jobs freely. The semaphore gates execution. The dispatcher can continue processing the queue (e.g., handling retries, checking cancel) while workers are busy.

**Why `spawn_blocking` and not async tasks?**
- git2-rs calls libgit2 C functions which perform synchronous I/O (network and filesystem).
- Running these on Tokio's async worker threads would block them, starving the runtime of available workers for other async tasks (event bus draining, UI tick).
- `tokio::task::spawn_blocking` moves the work to a dedicated thread pool (default size: 512, configurable). The async task awaits the blocking result without consuming a Tokio worker.

**Concurrency guideline:** The semaphore limit should be 8–16 for SSDs, 4–8 for HDDs. The user can configure this in Settings. The default is 8. Setting it too high causes disk thrashing (random I/O across many concurrent clone operations). Setting it too low leaves network bandwidth idle.

---

## 5. Retry System

### 5.1 Retriable vs Non-Retriable Errors

Not all failures should be retried. The retry system classifies errors into two categories:

| Error Category | Retriable? | Examples | Rationale |
|---------------|------------|----------|-----------|
| **Network transient** | Yes | DNS timeout, connection reset, TLS handshake failure, git fetch timeout | These often resolve on retry |
| **Rate limiting** | Yes | GitHub secondary rate limit, server-side throttling | Wait + retry |
| **Server errors** | Yes | HTTP 500, 502, 503 from git server | May be transient |
| **Disk full** | No (stop entire sync) | `ENOSPC` — no space left on device | Affects ALL jobs, not just this one. Pause the entire sync. |
| **Authentication** | No | 401, 403 — token expired or revoked | Won't resolve on retry. Fail permanently. |
| **Repository not found** | No | 404 — repo deleted or renamed | Won't resolve on retry (unless it's a transient GitHub issue, which is a server error). |
| **Permission denied** | No (stop entire sync) | Cannot write to clone path | Affects ALL jobs. |
| **SSH key error** | No | Invalid key, key not found, host key verification failed | Won't resolve on retry. |
| **Local git corruption** | Partial | Corrupt object database | Retry with a fresh clone (discard corrupt repo). Retry ONCE. |

### 5.2 Retry Decision Engine

The result collector task evaluates each `JobOutcome::Failed` and decides whether to retry:

```
fn should_retry(error: &SyncError, attempt: u32, max_retries: u32) -> RetryDecision {
    if attempt >= max_retries {
        return RetryDecision::RetriesExhausted;
    }

    match error {
        SyncError::Network(_) => {
            RetryDecision::Retry {
                delay: exponential_backoff(attempt, Duration::from_secs(5), 2.0, Duration::from_secs(60)),
                reason: "Network error — retrying...",
            }
        }
        SyncError::Git(GitError::RemoteRefused) => {
            // Server-side 5xx or rate limit
            RetryDecision::Retry {
                delay: exponential_backoff(attempt, Duration::from_secs(10), 2.0, Duration::from_secs(120)),
                reason: "Remote server error — retrying...",
            }
        }
        SyncError::DiskFull(_) => {
            RetryDecision::AbortAll {     // Pause entire sync
                reason: "Disk full. Free up space and resume, or cancel.",
            }
        }
        SyncError::Auth(_) => {
            RetryDecision::NoRetry {
                reason: "Authentication failed. Token may be expired.",
            }
        }
        SyncError::Git(GitError::RepositoryNotFound) => {
            RetryDecision::NoRetry {
                reason: "Repository not found on GitHub.",
            }
        }
        SyncError::Permissions(_) => {
            RetryDecision::AbortAll {
                reason: "Write permission denied. Check directory permissions.",
            }
        }
        SyncError::Git(GitError::ObjectCorrupt) => {
            if attempt == 0 {
                // First failure: delete the corrupt clone and retry as a fresh clone
                RetryDecision::RetryWithCleanSlate {
                    delay: Duration::from_secs(1),
                    reason: "Corrupt local repository — re-cloning from scratch.",
                }
            } else {
                RetryDecision::NoRetry {
                    reason: "Repository is corrupt even after re-clone.",
                }
            }
        }
        _ => {
            RetryDecision::NoRetry {
                reason: "Unexpected error.",
            }
        }
    }
}

fn exponential_backoff(
    attempt: u32,
    base: Duration,
    multiplier: f64,
    cap: Duration,
) -> Duration {
    let delay = base.mul_f64(multiplier.powi(attempt as i32));
    std::cmp::min(delay, cap)
}
```

**Backoff sequence:** 5s → 10s → 20s → 40s → capping at 60s. With `max_retries = 2` (default), the maximum retry delay is 10s (attempt 2).

### 5.3 Global Abort vs Local Failure

Some errors affect all jobs, not just one. The sync engine distinguishes:

- **Local failure:** Only this job is affected. Retry or skip. Other workers continue.
- **Global abort:** All jobs are affected. Pause the dispatcher. Emit a `SyncPaused` event with the reason. The user can free disk space and resume, or cancel.

Global abort triggers include:
- `ENOSPC` (disk full)
- `EACCES` / `EPERM` (permission denied on the clone path)
- Multiple consecutive network errors suggesting broader connectivity loss (configurable threshold: 5 failures across different repos within 30 seconds)

**State on global abort:**
- Dispatcher stops dequeuing new jobs.
- Active workers are allowed to complete (they might succeed if they already have data).
- The pause flag is set to `true`.
- The UI shows: "⏸ PAUSED — Disk full. 12.3 GB needed, 87 MB available. Free up space and press r to resume or c to cancel."

### 5.4 Retry Queue Ordering

When a job enters the retry queue, it's delayed by its backoff period. When the delay expires, it's re-enqueued.

**Ordering strategy:** Retried jobs are inserted at the **front** of the pending queue. This means:
- They get priority over jobs that haven't had their first attempt yet.
- A slow-retrying job doesn't get "leapfrogged" indefinitely by fast new jobs.
- Failed jobs are resolved quickly (either succeed on retry or hit max retries and terminate).

---

## 6. Progress Tracking

### 6.1 Progress Data Flow

Progress data flows through three channels:

```
git2-rs transfer_progress callback
  │  (called by libgit2 C code every ~100ms during fetch/clone)
  │
  ▼
mpsc::Sender<ProgressSample>  (inside spawn_blocking)
  │  (sends progress from blocking thread to async context)
  │
  ▼
tokio::sync::mpsc::UnboundedSender<AppEvent>  (progress_tx)
  │  (sends to main loop)
  │
  ▼
app::reduce_event()
  │  (updates SyncCenterTabState.live_log + overall_progress)
  │
  ▼
ui::render()
  │  (reads SyncCenterTabState, renders progress bars and live log)
```

### 6.2 Progress Sample

The minimum unit of progress data:

```
struct ProgressSample {
    job_id: JobId,
    repo_full_name: String,
    phase: JobPhase,
    indexed_objects: u32,        // objects indexed
    received_objects: u32,       // objects fetched
    total_objects: u32,          // total objects to fetch
    received_bytes: usize,       // bytes fetched so far
    timestamp: Instant,
}
```

This maps directly to git2-rs's `TransferProgress` struct with `indexed_objects`, `received_objects`, `total_objects`, `received_bytes`.

### 6.3 Progress Aggregation

The `ProgressTracker` aggregates per-job samples into overall metrics:

**Overall completion %:**
```
overall_percent = (completed_jobs + sum(active_job_percents)) / total_jobs
```
Where `active_job_percent = min(received_objects / total_objects, 1.0)` for each active job.

**Throughput (sliding window):**
```
window = last 5 seconds of byte samples (all active jobs aggregated)
throughput = sum(window_bytes) / 5.0  // bytes per second
```

**ETA:**
```
remaining_bytes = bytes_total_estimate - bytes_transferred
eta = remaining_bytes / throughput
```

**Why `bytes_total_estimate` is approximate:** We don't know the exact transfer size until the clone completes. The estimate is the sum of `size_kb * 1024` from the GitHub API. This underestimates (git objects compress differently than the API-reported size) but gives a reasonable ballpark ETA.

### 6.4 Progress Event Throttling

git2-rs calls the transfer progress callback very frequently (potentially every few milliseconds during fast clones). Sending an `AppEvent` on every callback would flood the event bus and cause the UI to re-render 100+ times per second.

**Throttling strategy:**
- The worker maintains a `last_emit: Instant` for each active job.
- Progress events are emitted at most once every **100ms** per job.
- If a progress update arrives before 100ms, it's buffered. When 100ms elapses, the latest buffered sample is sent.
- This means the UI updates at ~10 fps per active job, which is smooth enough for a progress bar while keeping the event bus load manageable.

```
fn should_emit_progress(last_emit: Instant, now: Instant) -> bool {
    now.duration_since(last_emit) >= Duration::from_millis(100)
}
```

### 6.5 Live Log Ring Buffer

The `SyncCenterTabState.live_log` is a ring buffer of `LogEntry` items. It is NOT the same as the `sync_log` database table (which stores all entries for the activity log). The live log is an in-memory, UI-only construct.

```
struct LogEntry {
    job_id: JobId,
    repo_full_name: String,
    status: LogEntryStatus,
    detail: String,          // "cloning... 45% · 54 MB / 120 MB"
    timestamp: Instant,
}

enum LogEntryStatus {
    Queued,
    Active,        // in progress (shown with spinner)
    Success,       // completed successfully
    Failed,        // errored
    Skipped,       // no-op
    Retrying,      // waiting to retry
}
```

**Ring buffer rules:**
- Max 500 entries. Oldest entries are dropped when the buffer is full.
- Each job has exactly one entry in the buffer (updated in-place, not appended).
- The entry's `status` transitions as the job progresses through its state machine.
- Completed entries (Success, Failed, Skipped) are rendered with a fixed ✓/✗/— icon and scroll off-screen naturally.
- Active entries are rendered with a spinner that advances on each tick.

---

## 7. Cancellation

### 7.1 Cancellation Signal

Cancellation is triggered by the user pressing `c` in Sync Center (with confirmation), or by `q` (quit) which cancels the sync before exiting.

**Signal mechanism:** A single `tokio::sync::watch::Sender<bool>` owned by the main loop. When the user cancels:
1. `cancel_tx.send(true)` (or `cancel_tx.send_modify(|v| *v = true)`)
2. All tasks that hold a `cancel_rx` clone will see `*cancel_rx.borrow() == true` on their next check.

### 7.2 Cancellation Propagation

Different components respond to cancellation at different speeds:

| Component | Cancellation Mechanism | Latency |
|-----------|----------------------|---------|
| **Dispatcher** | Checks `cancel_rx.borrow()` at the top of each loop iteration | <50ms (the loop sleep) |
| **Active git2-rs operation** | A separate approach: the `cancel_rx` is passed into the `spawn_blocking` closure. git2-rs has a `Remote::stop()` method. We call `remote.stop()` when cancellation is detected. | <100ms (libgit2 checks cancellation between network round-trips) |
| **Retry queue** | Retry entries are not dequeued once cancelled | Immediate — dispatcher stops processing |
| **Pending queue** | Jobs not yet dispatched are dropped | Immediate — dispatcher stops |

### 7.3 In-Progress Job Handling

When cancellation fires mid-clone:
1. The `spawn_blocking` closure detects `cancel_rx.borrow() == true`.
2. It calls `remote.stop()` on the git2-rs `Remote` handle, which terminates the network transfer.
3. git2-rs returns a `CloneStats` with `cancelled = true` (our custom wrapper adds this).
4. The worker returns `JobOutcome::Cancelled { partial_result: Some(...) }`.
5. **The partially cloned directory is left on disk.** It's marked in the DB as `clone_status = 'error'` with `last_error = "Cancelled by user"`. On the next sync, the engine will detect the partial clone and either resume it (future feature) or delete and re-clone.
6. The partial result's `bytes_transferred` is still counted in the final summary.

### 7.4 Post-Cancellation State

After all active workers drain (or timeout after 30 seconds), the collector task emits `AppEvent::SyncCancelled`:

```
AppEvent::SyncCancelled {
    summary: PartialSyncSummary {
        completed: 34,
        failed: 2,
        cancelled: 3,
        pending_dropped: 8,
        bytes_transferred: 1_200_000_000,
    },
}
```

The UI shows the partial results screen: "Sync cancelled. 34 of 47 repos processed. 3 were mid-operation and may have partial data."

### 7.5 Force Kill

If the user presses `q` (quit) during a sync, the app must exit. The cancel signal is sent, but the app does not wait indefinitely for workers to drain. A forced timeout of 5 seconds is used:

```
// In main loop quit handler:
cancel_tx.send(true)?;

// Wait up to 5s for sync task to acknowledge
let sync_task_handle: JoinHandle<_> = /* ... */;
match tokio::time::timeout(Duration::from_secs(5), sync_task_handle).await {
    Ok(_) => {} // Sync task cleaned up
    Err(_) => {
        // Timeout: just exit. Partial data in SQLite is fine.
        log::warn!("Sync did not terminate within 5s. Forcing exit.");
    }
}
```

---

## 8. Pause / Resume

### 8.1 Pause Mechanism

Pause is distinct from cancellation. Pause temporarily halts new work but preserves the queue state.

**Pause signal:** `Arc<AtomicBool>` — shared between the dispatcher and the UI. When the user presses `p`:
1. `pause_flag.store(true, Ordering::Release)`
2. The dispatcher detects this, stops dequeuing, and waits.
3. Active workers complete their current operations normally. They do NOT check the pause flag — only the dispatcher does.
4. The UI shows "⏸ PAUSED" and changes the key bar.

**Why allow active workers to finish?**
- Interrupting an in-progress clone loses all transferred data (the partial clone is useless without the full object database).
- Clone operations can take minutes for large repos. Losing that progress is worse than waiting for completion.
- The user's expectation of "pause" is that work stops soon — not immediately. Letting active ops complete typically takes <30 seconds for the slowest running job.

### 8.2 Resume Mechanism

When the user presses `r` (resume):
1. `pause_flag.store(false, Ordering::Release)`
2. The dispatcher wakes from its wait loop.
3. It resumes dequeuing from the pending queue and retry queue.
4. The UI transitions back to the active progress screen.

### 8.3 Pause During Long Operations

If all workers are active and the user pauses, the UI shows:
```
⏸ PAUSED — Finishing 3 active operations (acme/mobile-app, acme/terraform-infra, acme/auth-service)...
```

When all active workers complete and no new jobs are dispatched, the UI shows:
```
⏸ PAUSED — 13 jobs remaining. Press r to resume or c to cancel.
```

### 8.4 Automatic Pause Triggers

The engine may auto-pause in these conditions:
- **Disk full:** `ENOSPC` error → pause with message advising the user to free space.
- **Connectivity loss:** 3+ consecutive network errors within 60 seconds → pause with message "Network appears unstable. Pausing. Press r to retry or c to cancel."
- **GitHub rate limit (extreme):** If the GitHub integration layer signals that the rate limit is exhausted and won't reset for >5 minutes, pause rather than retry-loop.

---

## 9. Coordinator Task — Putting It All Together

The coordinator task is the entry point. It constructs all sub-components and oversees the pipeline:

```
async fn run_sync(
    plan: SyncPlan,
    git: Arc<dyn GitOps>,
    db: Arc<dyn Database>,
    progress_tx: mpsc::UnboundedSender<AppEvent>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
    cancel_rx: watch::Receiver<bool>,
) {
    // ── 1. Pre-flight: resolve clone URLs, determine operations ──
    let jobs = resolve_plan(plan); // PlannedOperation::Clone | Sync | Skip

    // ── 2. Initialize components ──
    let pause_flag = Arc::new(AtomicBool::new(false));
    let semaphore = Arc::new(Semaphore::new(plan.concurrency));
    let tracker = Arc::new(ProgressTracker::new(jobs.len()));
    let (result_tx, mut result_rx) = mpsc::channel(plan.concurrency * 2);
    let queue = JobQueue::new(jobs, pause_flag.clone(), cancel_rx.clone());

    // ── 3. Emit sync started event ──
    let _ = app_tx.send(AppEvent::SyncAllStarted {
        run_id: plan.run_id,
        total_jobs: queue.total(),
    });

    // ── 4. Spawn dispatcher ──
    let dispatch_handle = tokio::spawn(dispatcher(
        queue, semaphore, git, tracker.clone(),
        plan.options.clone(), result_tx,
        progress_tx.clone(), cancel_rx.clone(),
    ));

    // ── 5. Spawn result collector ──
    let collector_handle = tokio::spawn(result_collector(
        result_rx, tracker, plan.options,
        progress_tx.clone(), app_tx.clone(), cancel_rx.clone(),
    ));

    // ── 6. Wait for pipeline to complete ──
    tokio::select! {
        _ = dispatch_handle => {},
        _ = collector_handle => {},
        _ = cancel_rx.changed() => {
            // Cancellation — wait for drain
            let _ = tokio::time::timeout(
                Duration::from_secs(30),
                futures::future::join(dispatch_handle, collector_handle),
            ).await;
        }
    }

    // ── 7. Persist sync run to SQLite ──
    // (The collector task already accumulated the summary;
    //  the coordinator task writes it to the database)
    let _ = db.update_sync_run(plan.run_id, &summary).await;

    // ── 8. Emit final event ──
    let _ = app_tx.send(AppEvent::SyncAllCompleted { summary });

    // ── 9. Trigger stats refresh (so Dashboard updates) ──
    let _ = app_tx.send(AppEvent::StatsRefreshed { /* ... */ });
}
```

---

## 10. Failure Recovery

### 10.1 Crash Recovery

If the application crashes (or is killed) during a sync:

**What survives:**
- All previously successful sync runs → stored in `sync_runs` + `sync_entries` in SQLite.
- All cloned repos → on disk at `~/git-vacuum/<owner>/<repo>/`. They're valid git repos.
- Partially cloned repos → on disk with incomplete git data. They are NOT valid git repos yet.

**What is lost:**
- The current in-progress sync run row → `status = 'running'` in `sync_runs` table. The next startup should either resume it (future feature) or mark it as `cancelled`.
- The live log → it was in-memory only.

**Recovery on next startup:**
1. SQLite: mark all `sync_runs` with `status = 'running'` as `status = 'cancelled'` (they were interrupted).
2. Filesystem: check each local clone path. If `is_git_repo(path) == false`, mark it as `clone_status = 'error'` with `last_error = 'Interrupted clone. Re-clone on next sync.'`.
3. If `is_git_repo(path) == true`, run `git.fetch()` to update `behind_count`. This gives the Dashboard accurate data without a full discovery.

### 10.2 Stale Lock Files

Git operations can leave behind lock files (`.git/index.lock`, `.git/HEAD.lock`) if they're forcefully killed. On next startup, the engine checks for and removes stale lock files (files older than 15 minutes with no active git process referencing them).

### 10.3 Disk Space Recovery Strategy

The engine does NOT automatically delete repos to free space. This is an explicit user action (prune). If disk space runs out:
1. The engine pauses.
2. The UI shows which repos succeeded and how much space is needed.
3. The user can cancel, free space externally, and resume — or prune repos from the Dashboard.
4. If the user prunes and resumes, the freed repos are skipped in the remaining queue.

### 10.4 Network Intermittency

For repos behind unstable connections, the retry system handles transient failures. But if the network is permanently down, the engine should not retry forever.

**Circuit breaker:** If 5+ consecutive jobs fail with network errors within a 60-second window, the engine auto-pauses with the message: "Network appears unstable. 5 consecutive failures. Pausing. Check your connection and press r to retry."

---

## 11. Key Design Decisions (Rationale)

### Decision 1: Staged pipeline over `join_all`
**Chosen:** Four-stage pipeline with channel-based communication.  
**Rejected:** `futures::future::join_all(spawned_tasks)`  
**Why:** A flat `join_all` can't support pause/resume, can't handle retries with backoff, and would emit all completion events simultaneously (or worse, none until all complete). The staged pipeline gives us control at each transition point.

### Decision 2: Semaphore over bounded channel for concurrency
**Chosen:** `tokio::sync::Semaphore` for worker concurrency.  
**Rejected:** `mpsc::channel(concurrency)` to submit jobs to workers.  
**Why:** A bounded channel couples job submission to worker availability — the dispatcher blocks on send. A semaphore decouples them — the dispatcher dequeues freely, the semaphore gates execution. This matters for retry handling: we can check the retry queue and cancel flag even when all workers are busy.

### Decision 3: Retry at the engine level, not the git level
**Chosen:** Retry decisions are made by the result collector (engine level), not by individual workers.  
**Rejected:** Each worker implementing its own retry loop.  
**Why:** Centralized retry logic can make global decisions (abort all on disk full, circuit breaker on network instability). Per-worker retry loops can't coordinate with each other.

### Decision 4: Allow active jobs to complete on pause
**Chosen:** Pause only stops new dispatches. Active jobs finish.  
**Rejected:** Pause interrupts all jobs immediately.  
**Why:** Interrupting a clone that's 95% complete wastes all transferred data. Letting it finish takes a few more seconds and preserves progress. The user can always cancel if they want immediate termination.

### Decision 5: Partial clone left on disk on cancel
**Chosen:** Keep partial data on disk, marked as errored.  
**Rejected:** Delete partial data immediately.  
**Why:** The partial data represents transferred bytes that won't need to be re-fetched. In the future, we can implement resume-clone (git supports shallow clone + fetch to complete). Until then, marking as errored and re-cloning on next sync is equivalent to deleting, but with better diagnostics.

### Decision 6: Priority queue for job dispatch
**Chosen:** Small repos first, large repos later. Retries at the front.  
**Rejected:** FIFO order as received.  
**Why:** Small repos complete quickly, giving the user rapid visual progress (the progress bar jumps from 0% to 40% fast). Large repos dominate later. This is a well-known UX optimization: early progress builds confidence that the sync is working.

---

## 12. Sync Engine Module Structure

```
src/service/sync_engine/
├── mod.rs                  # Re-exports, run_sync() entry point
├── coordinator.rs          # Coordinator task: component wiring, lifecycle
├── plan.rs                 # SyncPlan, JobSpec, PlannedOperation, resolve_plan()
├── queue.rs                # JobQueue, dispatch logic, priority ordering
├── dispatcher.rs           # Dispatcher loop: dequeues, spawns workers, pause/cancel
├── worker.rs               # execute_job(): clone, sync, mirror per job
├── retry.rs                # should_retry(), RetryDecision, backoff calculation
├── collector.rs            # Result collector: aggregates outcomes, decides retries
├── progress.rs             # ProgressTracker: aggregation, throughput, ETA
├── events.rs               # Sync engine-specific AppEvent variants (detailed)
└── error.rs                # SyncError, error classification (retriable vs not)
```
