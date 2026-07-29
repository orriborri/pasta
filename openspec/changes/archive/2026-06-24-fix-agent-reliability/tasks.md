## 1. Agent Timeout

- [x] 1.1 Add `agent_timeout_minutes: u64` to `GeneralConfig` in `config.rs` with default 10
- [x] 1.2 In `process.rs` `spawn_agent`, wrap `child.wait()` with `tokio::time::timeout` using the config value; on timeout, kill the child and log "TIMEOUT — killed"

## 2. Scheduler Lock Narrowing

- [x] 2.1 In `run_agentic_schedules`, collect spawn decisions into a Vec while holding `sched_state`, then drop the lock before spawning processes

## 3. Log Cleanup

- [x] 3.1 In `vault_manager.rs`, add `cleanup_old_logs()` that deletes `.log` files in `./logs/` with mtime older than 7 days

## 4. ACP Hardcoded Path

- [x] 4.1 In `acp.rs`, replace hardcoded `"/home/orre/Obsidian/Readpeak"` with `pasta_common::vault::vault_path()`
