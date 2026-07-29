## Why

The pasta-backend `main.rs` has grown to 600+ lines handling 5+ distinct responsibilities (CLI parsing, socket server, scheduler, command dispatch, ACP protocol). The fetch cycle is duplicated between the scheduler and `ForceFetch` command with behavioral differences. Hardcoded paths, unsafe PID tracking, and swallowed errors make the system fragile and untestable. Refactoring now prevents further accretion and enables testing individual components.

## What Changes

- Split `main.rs` into focused modules: `scheduler.rs`, `server.rs`, `acp.rs`, `cli.rs`
- Extract duplicated fetch cycle into a single reusable `run_fetch_cycle()` function
- Split `AppState` into focused sub-states (scheduling, connections, process tracking)
- Inject `CommandRunner` into fetchers instead of calling `run_cmd` directly
- Replace unsafe `libc::waitpid` PID tracking with `tokio::process::Child` JoinHandles
- Make `run_sync` accept a trait object for progress reporting instead of constructing `SyncProgress` internally
- Move hardcoded binary paths to config/environment resolution
- Replace silent `.ok()` error swallowing with `tracing::error!` logging

## Capabilities

### New Capabilities
- `module-structure`: Decomposition of main.rs into single-responsibility modules with clear boundaries
- `dependency-injection`: Trait-based injection for command execution and progress reporting, enabling testability
- `process-management`: Safe async process tracking replacing unsafe PID-based waitpid

### Modified Capabilities

## Impact

- `crates/backend/src/main.rs` — split into multiple files
- `crates/backend/src/fetchers/mod.rs` — `CommandRunner` injected into fetcher functions
- `crates/backend/src/history/mod.rs` — `run_sync` signature changes to accept progress trait
- `crates/backend/src/progress.rs` — no structural change, but consumed differently
- No API/IPC protocol changes — TUI remains unaffected
- No dependency additions expected (tokio already provides JoinHandle)
