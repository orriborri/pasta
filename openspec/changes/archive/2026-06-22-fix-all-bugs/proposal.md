## Why

The pasta backend has critical data-loss race conditions in state persistence, blocking async calls degrading concurrency, and content-destroying behavior in daily note generation. These bugs cause duplicate indexing (wasted API calls), reduced throughput during Slack sync, and silent loss of user-written content.

## What Changes

- Fix race condition in `persist_hashes` and `fetch_cycle` state save/load that causes data loss and duplicate indexing
- Fix `run_sync` state merge overwriting concurrent changes
- Replace blocking `std::thread::sleep` with `tokio::time::sleep` in `sync_slack`
- Fix ACP initialization to wait for responses instead of fixed sleeps
- Fix daily note `generate_daily` truncating content below the activity feed section
- Fix `cleanup_old_logs` using relative path instead of configured log directory
- Use `resolve_binary` for LINEAR_API in history/sync_linear instead of hardcoded path
- Fix fetch cycle marking fetchers as completed even on failure
- Fix `flag_stale_tasks` never un-marking stale status

## Capabilities

### New Capabilities
- `state-persistence`: Atomic state persistence with file locking to prevent race conditions

### Modified Capabilities
- `daemon-singleton`: Fix log cleanup path and fetch-cycle completion signals

## Impact

- `crates/backend/src/history/indexer.rs` — atomic hash persistence
- `crates/backend/src/history/state.rs` — file-locked save_state
- `crates/backend/src/history/mod.rs` — state merge fix
- `crates/backend/src/history/sync_slack.rs` — async sleep
- `crates/backend/src/history/sync_linear.rs` — resolve_binary usage
- `crates/backend/src/history/vault_organize.rs` — daily note section handling
- `crates/backend/src/fetch_cycle.rs` — conditional completion marking, state save fix
- `crates/backend/src/acp.rs` — response-awaiting initialization
- `crates/backend/src/fetchers/vault_manager.rs` — log dir path fix, stale task logic
