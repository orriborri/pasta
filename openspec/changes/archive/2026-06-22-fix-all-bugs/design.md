## Context

The pasta backend daemon has state persistence that uses simple load→modify→save cycles without any concurrency protection. Multiple async tasks (fetch_cycle, history sync, indexer batches) write to the same `_sync_state.json` file concurrently, causing last-writer-wins data loss. Additionally, blocking thread sleeps in async contexts reduce concurrency, and the daily note generator destroys user content on updates.

## Goals / Non-Goals

**Goals:**
- Eliminate state corruption from concurrent writes to `_sync_state.json`
- Ensure indexer hash persistence is atomic and never overwritten by stale snapshots
- Replace all blocking sleeps with async equivalents in sync_slack
- Make ACP initialization response-driven rather than timing-dependent
- Preserve user-written content in daily notes across regeneration
- Fix minor bugs: relative log path, hardcoded binary, unconditional completion marking, stale task lifecycle

**Non-Goals:**
- Redesigning the state persistence to use a database (SQLite, etc.)
- Adding multi-client TUI support
- Fixing the embedding dimension mismatch (requires reindexing)
- Restructuring the TUI codebase

## Decisions

### 1. File-locked state persistence

**Decision**: Use `flock(LOCK_EX)` around all `save_state` and `load_state` calls.

**Rationale**: The daemon is single-process with async tasks. A file lock prevents concurrent writers within the same process (across tokio tasks) and protects against any CLI invocations that might overlap. This is the minimal change — no new dependencies, no schema change.

**Alternative considered**: `tokio::sync::RwLock` wrapping state — rejected because CLI subcommands (`--sync-once`, `--reindex`) also read/write state from separate processes.

### 2. Remove stale-snapshot overwrites in fetch_cycle

**Decision**: Remove `history::state::save_state(&idx_state)` from `fetch_cycle.rs` since `index_new_files` already persists hashes internally. The caller should not save a pre-indexing snapshot.

**Rationale**: The indexer's `persist_hashes` is the authoritative writer for file hashes. Saving the old snapshot after indexing undoes those writes.

### 3. Async sleep for Slack rate limiting

**Decision**: Since `sync_slack::sync` is already `async fn`, replace `std::thread::sleep` with `tokio::time::sleep`. Also switch from `crate::fetchers::run_cmd` (blocking) to `tokio::process::Command` for Slack API calls within the async context.

**Rationale**: The current blocking sleep holds a tokio worker thread. Since the function is already async and called within `tokio::join!`, using async sleep is the correct approach. The helper functions (`fetch_all_history`, `resolve_user`) need to become async.

**Alternative considered**: `spawn_blocking` wrapper — rejected as it would still block a thread from the blocking pool and adds unnecessary complexity when the function is already async.

### 4. Response-awaiting ACP initialization

**Decision**: After sending `initialize` and `session/new`, read from stdout until we receive a JSON-RPC response with matching `id`. Use a timeout of 10 seconds.

**Rationale**: Fixed sleeps are fragile. Reading the response ensures the agent is actually ready before sending prompts.

### 5. Daily note section preservation

**Decision**: When replacing the activity feed section, find the section end (next `## ` heading or EOF) and preserve content after it.

**Rationale**: Users add notes below the activity feed. The current `split("## 🔗 Activity Feed").next()` discards everything after the heading.

### 6. Conditional fetch completion

**Decision**: Track success/failure per fetcher. Only insert into `completed` set if the fetch actually returned data (non-empty result).

**Rationale**: Triggering dependent agents on failed fetches causes them to process stale data.

## Risks / Trade-offs

- [File lock contention] → Lock is held only during JSON serialization/write (~ms). Acceptable for the current access pattern.
- [Making sync_slack fully async] → Requires converting helper functions. Risk of missed rate limiting if `tokio::time::sleep` is accidentally removed. → Mitigated by keeping sleep calls explicit and co-located with API calls.
- [ACP response timeout] → If kiro-cli takes >10s to initialize, chat will fail. → 10s is generous; can be made configurable later.
