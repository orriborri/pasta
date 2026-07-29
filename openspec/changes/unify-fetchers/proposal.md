## Why

The backend has two separate fetcher systems calling the same external APIs (Slack, Gmail, Linear, GitLab, Calendar): lightweight "backend fetchers" for TUI feeds and heavier "kb-fetchers" for search indexing. This means every fetch cycle hits rate-limited APIs twice, the backend fetchers lack pagination/cursor-tracking/retry logic that kb-fetchers already have, and `kb-sync` runs as a separate shell-out schedule adding latency before data is searchable. Unifying to a single fetch pass eliminates redundant API calls, gives the TUI richer data, and makes search indexing immediate.

## What Changes

- Replace backend fetchers (`crates/backend/src/fetchers/{slack,gmail,linear,gitlab,calendar}.rs`) with calls to kb-fetchers
- Derive TUI `.feeds/` markdown from kb-fetcher `Record` output instead of bespoke format functions
- Run kb-sync pipeline inline in the fetch cycle (already started: `kb-sync` crate extracted)
- Remove the standalone `kb-sync` native schedule (no longer needed)
- Remove the `run_kb_sync` shell-out function from the scheduler
- Keep `vault_manager.rs` unchanged (different concern — vault maintenance, not API fetching)

## Capabilities

### New Capabilities
- `unified-fetch`: Single fetch pass using kb-fetchers for both TUI feeds and search indexing

### Modified Capabilities
- `ingestion-pipeline`: Pipeline now runs inline within the fetch cycle rather than in a separate `kb sync` process

## Impact

- `crates/backend/src/fetchers/` — slack, gmail, linear, gitlab, calendar modules replaced by thin feed formatters over `Record`
- `crates/backend/src/fetch_cycle.rs` — rewritten to call kb-fetchers then derive feeds + run pipeline
- `crates/backend/Cargo.toml` — adds `kb-fetchers`, `kb-sync` deps; may drop some now-unused deps
- `crates/kb-sync/` — new shared crate (already scaffolded) encapsulating fetch→pipeline→store
- `crates/common/src/data.rs` — `KNOWN_NATIVE_SCHEDULES` drops `kb-sync`
- `crates/common/src/config.rs` — `kb-sync` schedule default removed
- `crates/backend/src/scheduler.rs` — `kb-sync` entry removed from REGISTRY
- Config files: users with `[schedules.kb-sync]` in config.toml get a harmless ignored entry
