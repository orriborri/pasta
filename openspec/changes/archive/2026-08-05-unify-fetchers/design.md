## Context

The backend currently has two fetcher codepaths:
1. **Backend fetchers** (`crates/backend/src/fetchers/{slack,gmail,linear,gitlab,calendar}.rs`) — stateless, synchronous, produce markdown feeds for the TUI dashboard. Simple but lack pagination, cursor-tracking, and retry logic.
2. **kb-fetchers** (`crates/kb-fetchers/src/{slack,gmail,linear,git,vault,calendar,gdocs}.rs`) — async, cursor-based incremental sync, rate-limit retries, pagination, produce typed `Record` structs for search indexing.

Both call the same CLI tools (`slack-api`, `gog`, `glab`, `linear-api`). The kb-fetchers run via a separate `kb-sync` schedule that shells out to `kb sync`, introducing a delay between fetching and indexing.

A `kb-sync` library crate has been scaffolded to share the sync logic between `kb-cli` and the backend.

## Goals / Non-Goals

**Goals:**
- Single fetch pass per cycle using kb-fetchers (eliminates duplicate API calls)
- TUI feeds derived from `Record` output (richer data, consistent format)
- Search indexing runs inline immediately after fetching (no separate schedule)
- Fewer moving parts: one fetch system, one data flow

**Non-Goals:**
- Changing the kb-fetcher implementations themselves
- Modifying the ingestion pipeline stages
- Replacing `vault_manager.rs` (different concern)
- Changing the TUI feed rendering (only the data source changes)
- Merging the `kb` CLI tool into the backend

## Decisions

### 1. kb-fetchers become the single source of truth for all external data

**Rationale**: They already have pagination, cursors, retries, and richer data extraction. The backend fetchers are a simpler subset that would need these features added anyway.

**Alternative considered**: Keep both and share a client layer. Rejected because it still means two fetch passes or complex result-sharing.

### 2. Feed markdown derived from `Vec<Record>` via format functions

**Rationale**: Records contain all data needed for feeds (author, content, timestamps, source). Thin format functions (`format_feed_from_records(source, records) -> String`) replace the current per-source `format_feed()` functions.

**Alternative considered**: Store Records and have TUI read them directly. Rejected — adds complexity to TUI; markdown feeds are simple and work.

### 3. `kb-sync` crate owns the full fetch→pipeline→store flow

**Rationale**: Both `kb` CLI and the backend need the same logic. A shared crate avoids duplication without coupling kb-cli to the backend or vice versa.

### 4. Fetch cycle calls `kb_sync::run()` which fetches AND indexes

**Rationale**: The fetch cycle already runs on a timer in the background. Adding indexing inline means data is searchable immediately. The operation is CPU/IO bound but not latency-sensitive (it's a background task).

### 5. Feed formatting moves to `kb-sync` crate alongside the fetch

**Rationale**: After fetching, the same records feed both the indexing pipeline and the `.feeds/` markdown generation. Keeping both in `kb-sync::run()` means the fetch cycle just calls one function and gets both outputs.

## Risks / Trade-offs

**[Risk] Fetch cycle becomes slower** → Acceptable since it runs in background. kb-fetchers do more work (pagination, full history) but that's gated by cursor state — incremental runs are fast.

**[Risk] Embedding model initialization blocks first fetch** → `embedder::init()` does a single probe call and uses `OnceLock`. After first init, it's a no-op.

**[Risk] `kb-sync` failure blocks feed updates** → Mitigate by running feed generation BEFORE the pipeline/store step. If indexing fails, feeds still update.

**[Risk] Users with `[schedules.kb-sync]` in config** → Harmless ignored entry. Add deprecation log on startup if detected.

**[Trade-off] Backend no longer has separate light-weight "current state" fetch** → kb-fetchers with cursors will only return NEW records. For the "current unread" TUI view, we may need to keep a small read of current state. Investigate if `SyncState` cursors cause gaps in the feed view.
