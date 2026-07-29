## Why

The pasta daemon has several critical bugs causing duplicate agent spawning, data corruption, and wasted resources. The most severe is a lack of single-instance enforcement — two daemons can run simultaneously, doubling all scheduled work. Additionally, `persist_hashes` overwrites disk state with stale data, and `update_task_dependencies` corrupts frontmatter.

## What Changes

- Add PID-file locking to prevent multiple daemon instances from running
- Fix `persist_hashes` to load fresh state from disk before merging
- Fix `update_task_dependencies` frontmatter insertion logic
- Remove hardcoded paths in `sync_slack.rs` and `acp.rs` — use config system
- Fix `mr_status` string concatenation producing malformed output

## Capabilities

### New Capabilities
- `daemon-singleton`: Enforce single daemon instance via PID-file locking on startup

### Modified Capabilities
- `unified-config`: Hardcoded binary paths in `sync_slack.rs` and `acp.rs` must use the existing `BinaryConfig` from `pasta.toml`

## Impact

- `crates/backend/src/main.rs` — add lock-file guard
- `crates/backend/src/history/indexer.rs` — fix `persist_hashes`
- `crates/backend/src/history/feed_history.rs` — fix frontmatter insertion
- `crates/backend/src/history/sync_slack.rs` — use config for binary paths
- `crates/backend/src/acp.rs` — use config for kiro-cli path
- `crates/backend/src/fetchers/gitlab.rs` — fix `mr_status`
