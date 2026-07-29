## Why

All sync sources (Gmail, Slack, Linear) can re-fetch already-fetched data when state is lost or on first run. Gmail specifically hangs trying to fetch 6 months at once. We need a consistent incremental strategy across all sources: fetch a small window first, then expand forward (new) and backward (older history) on each run, never overlapping already-fetched ranges.

## What Changes

- All sources use a `newest_fetched` / `oldest_fetched` window tracked in state
- First sync: fetch only last 7 days for any source
- Each subsequent sync: fetch new data since `newest_fetched` AND one week further back from `oldest_fetched`
- Never re-fetch date ranges already covered
- Add timeout to `run_cmd` (60s) to prevent hangs
- Log stderr from failed commands

## Capabilities

### New Capabilities
- `incremental-history-fetch`: Bidirectional incremental fetching for all sources — grows forward and backward with each sync, storing fetch boundaries in state. Never re-fetches already-covered ranges.

### Modified Capabilities

## Impact

- `crates/backend/src/history/sync_gmail.rs` — incremental window logic
- `crates/backend/src/history/sync_slack.rs` — apply same window pattern (replace 90/180 day fallback)
- `crates/backend/src/history/sync_linear.rs` — apply same window pattern
- `crates/backend/src/history/state.rs` — add `newest_fetched` / `oldest_fetched` to all source states
- `crates/backend/src/fetchers/mod.rs` — add timeout and stderr capture to `run_cmd`
