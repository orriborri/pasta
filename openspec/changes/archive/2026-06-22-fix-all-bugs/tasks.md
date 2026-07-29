## 1. State Persistence — File Locking

- [x] 1.1 Add `flock`-based exclusive locking to `save_state` in `crates/backend/src/history/state.rs`
- [x] 1.2 Add shared locking to `load_state` for safe concurrent reads
- [x] 1.3 Remove the stale `history::state::save_state(&idx_state)` call in `crates/backend/src/fetch_cycle.rs` (line 55-57) that overwrites indexer's hash writes

## 2. State Merge Fix in run_sync

- [x] 2.1 In `crates/backend/src/history/mod.rs`, reload state from disk after `tokio::join!` completes, then apply source-specific fields (slack/gmail/linear cursors) to the fresh state before saving

## 3. Async Slack Sync

- [x] 3.1 Convert `fetch_all_history` in `crates/backend/src/history/sync_slack.rs` to async, replacing `std::thread::sleep` with `tokio::time::sleep`
- [x] 3.2 Convert `resolve_user` to async with `tokio::time::sleep` for rate limiting
- [x] 3.3 Replace `crate::fetchers::run_cmd` calls in sync_slack with async `tokio::process::Command`

## 4. ACP Response-Driven Init

- [x] 4.1 In `crates/backend/src/acp.rs`, replace fixed sleeps after `initialize` and `session/new` with a loop that reads stdout for a JSON-RPC response matching the request ID, with 10s timeout
- [x] 4.2 Return error on timeout and kill the subprocess

## 5. Daily Note Content Preservation

- [x] 5.1 In `generate_daily` in `crates/backend/src/history/vault_organize.rs`, find the end of the activity feed section (next `## ` heading or EOF) and preserve content after it when replacing

## 6. Fetch Cycle Conditional Completion

- [x] 6.1 In `crates/backend/src/fetch_cycle.rs`, only insert fetcher names into `completed` set if their `spawn_blocking` result returned non-empty data (check `gl.is_ok()` and data is non-empty)

## 7. Minor Fixes

- [x] 7.1 In `crates/backend/src/fetchers/vault_manager.rs`, change `cleanup_old_logs` to use `crate::util::log_dir()` instead of relative `./logs`
- [x] 7.2 In `crates/backend/src/history/sync_linear.rs`, replace hardcoded `LINEAR_API` constant with `crate::fetchers::resolve_binary("linear-api", "LINEAR_API_PATH", "linear-api")`
- [x] 7.3 In `flag_stale_tasks`, remove `stale: true` from tasks that now have a `due:` field or whose status is no longer `todo`
