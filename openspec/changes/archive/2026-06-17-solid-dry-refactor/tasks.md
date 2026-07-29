## 1. Extract ACP Module

- [x] 1.1 Create `crates/backend/src/acp.rs` with `AcpHandle`, `spawn_acp()`, `parse_acp_notification()`, and `send_prompt()`
- [x] 1.2 Remove ACP code from main.rs, replace with `use acp::*`
- [x] 1.3 Verify `cargo build` passes

## 2. Extract Server Module

- [x] 2.1 Create `crates/backend/src/server.rs` with `handle_client()`, `send_event()`, and the accept loop as `pub async fn run()`
- [x] 2.2 Move command dispatch into `crates/backend/src/commands.rs` with `handle_command()`
- [x] 2.3 Wire main.rs daemon path to call `server::run()`
- [x] 2.4 Verify `cargo build` passes

## 3. Extract Scheduler and Fetch Cycle

- [x] 3.1 Create `crates/backend/src/fetch_cycle.rs` with `pub async fn run_fetch_cycle()` consolidating the duplicated logic
- [x] 3.2 Create `crates/backend/src/scheduler.rs` with the tick loop calling `run_fetch_cycle()` and evaluating agent schedules
- [x] 3.3 Update `commands.rs` `ForceFetch` arm to call `run_fetch_cycle()`
- [x] 3.4 Verify both scheduled and manual fetches produce identical side effects (feeds + history + index)

## 4. Split AppState

- [x] 4.1 Define `SchedulerState`, `ProcessState`, `ConnectionState` structs in `crates/backend/src/state.rs`
- [x] 4.2 Replace single `Arc<Mutex<AppState>>` with separate `Arc<Mutex<_>>` for each sub-state
- [x] 4.3 Update all call sites to acquire only the lock they need
- [x] 4.4 Verify no deadlocks by establishing consistent lock ordering (scheduler → process → connection)

## 5. Dependency Injection for Fetchers

- [x] 5.1 Change `gitlab::fetch_raw()` signature to accept `&dyn CommandRunner`, pass it through
- [x] 5.2 Apply same pattern to `linear::fetch_raw()`, `slack::fetch_raw()`, `gmail::fetch_raw()`
- [x] 5.3 Create `resolve_binary(name: &str, env_key: &str) -> Result<PathBuf>` utility replacing hardcoded paths
- [x] 5.4 Update `run_fetch_cycle()` to construct a `SystemCommandRunner` and pass it to fetchers
- [x] 5.5 Add a unit test for `gitlab::fetch_raw()` with a mock runner returning fixture JSON

## 6. Safe Process Management

- [x] 6.1 Replace `RunningMap = Arc<Mutex<HashMap<usize, u32>>>` with `HashMap<usize, JoinHandle<ExitStatus>>`
- [x] 6.2 Rewrite `spawn_agent()` to return a `JoinHandle` that awaits the child process
- [x] 6.3 Rewrite `cleanup_finished()` to poll JoinHandles with `is_finished()` instead of `libc::waitpid`
- [x] 6.4 Remove the `unsafe` block entirely
- [x] 6.5 Add graceful shutdown: on SIGTERM, signal all children and await with 5s timeout

## 7. Progress Injection for run_sync

- [x] 7.1 Change `run_sync()` signature to accept `&dyn StageProgress + IndexProgress` (or a combined trait)
- [x] 7.2 Create `NoopProgress` implementation for daemon use
- [x] 7.3 Pass `SyncProgress::new()` from CLI paths, `NoopProgress` from scheduler

## 8. Error Observability

- [x] 8.1 Replace all `.ok()` calls in fetch cycle with `if let Err(e) = ... { tracing::warn!(...) }`
- [x] 8.2 Replace `.ok()` calls in scheduler with `tracing::error!` for critical failures
- [x] 8.3 Verify `cargo clippy` passes with no new warnings

## 9. Final Cleanup

- [x] 9.1 Slim main.rs to CLI parsing only — verify it's under 80 lines
- [x] 9.2 Run full `cargo build --release` and verify binary works end-to-end
- [x] 9.3 Run `--sync-once` and verify tracing output shows proper error/warn messages
