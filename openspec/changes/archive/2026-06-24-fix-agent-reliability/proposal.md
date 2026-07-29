## Why

Spawned kiro-cli agents sometimes hang indefinitely (e.g., waiting for browser auth with no TTY), blocking their scheduler slot forever. There's no timeout, no log cleanup, and the ACP chat has a hardcoded `current_dir`. On bad days this produced 400KB spinner logs and doubled costs from stuck processes.

## What Changes

- Add a configurable timeout (default 10 minutes) to `spawn_agent` that kills hung processes
- Add log rotation: delete agent logs older than 7 days on each vault-maintenance cycle
- Replace hardcoded `current_dir` in `acp.rs` with `vault_path()` from config
- Reduce scheduler lock scope in `run_agentic_schedules` to avoid holding `sched_state` across `process` lock

## Capabilities

### New Capabilities
- `agent-timeout`: Enforce a maximum runtime on spawned agent processes with automatic kill and error reporting

### Modified Capabilities
- `unified-config`: Add `[general] agent_timeout_minutes` config field (default 10)

## Impact

- `crates/backend/src/process.rs` — wrap `child.wait()` with `tokio::time::timeout`, kill on expiry
- `crates/backend/src/scheduler.rs` — narrow lock scope in `run_agentic_schedules`
- `crates/backend/src/fetchers/vault_manager.rs` — add log cleanup to maintenance cycle
- `crates/backend/src/acp.rs` — use `vault_path()` for `current_dir`
- `crates/common/src/config.rs` — add `agent_timeout_minutes` to `GeneralConfig`
