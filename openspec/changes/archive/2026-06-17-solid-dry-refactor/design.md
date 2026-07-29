## Context

pasta-backend is a Rust async daemon (~600 lines in main.rs) that orchestrates data fetching (GitLab, Linear, Slack, Gmail), history sync/indexing, agent scheduling, and an ACP chat bridge — all communicated to a TUI via Unix socket IPC. The codebase works but has grown organically, with responsibilities interleaved and duplication between the scheduler and manual command paths.

## Goals / Non-Goals

**Goals:**
- Each module has a single responsibility and can be understood in isolation
- Fetcher logic is testable without network access via injected `CommandRunner`
- Process management is safe (no unsafe blocks) and handles edge cases (PID reuse)
- Fetch cycle logic exists in one place, used by both scheduler and manual trigger
- Errors are observable via tracing, not silently swallowed
- External binary paths are configurable, not hardcoded

**Non-Goals:**
- Changing the IPC protocol or TUI behavior
- Adding new features or fetchers
- Changing the history sync algorithm or LanceDB schema
- Performance optimization (this is a structural refactor)

## Decisions

### 1. Module decomposition of main.rs

Split into:
- `main.rs` — CLI arg parsing + entrypoint only
- `server.rs` — UnixListener accept loop + client handler
- `scheduler.rs` — tick loop, schedule evaluation, fetch cycle orchestration
- `acp.rs` — ACP child process spawn, JSON-RPC protocol, notification parsing
- `commands.rs` — `handle_command()` dispatch logic
- `state.rs` — `AppState` definition split into sub-states

**Rationale**: Each file maps to one responsibility. The current main.rs requires reading 600 lines to understand any single concern.

### 2. Extract `run_fetch_cycle()` as single function

One async function called from both the scheduler and `Command::ForceFetch`. It handles: spawn fetchers → collect raw data → write feeds → write history → index → notify completion.

**Rationale**: Currently duplicated with behavioral differences (scheduler writes history, ForceFetch doesn't). Single function eliminates divergence.

### 3. Inject `CommandRunner` into fetchers

Change fetcher signatures from:
```rust
pub fn fetch_raw() -> GitlabData
```
to:
```rust
pub fn fetch_raw(runner: &dyn CommandRunner) -> GitlabData
```

**Rationale**: Enables unit testing with a mock runner. The trait already exists but isn't used.

### 4. Replace PID tracking with JoinHandle

Instead of storing `HashMap<usize, u32>` (index → PID) and using `libc::waitpid`, store `HashMap<usize, JoinHandle<()>>` wrapping a `tokio::process::Child`.

**Rationale**: Removes `unsafe`, eliminates PID reuse risk, integrates with tokio's task lifecycle.

### 5. Split AppState into focused structs

```rust
struct SchedulerState { last_native_fetch, last_vault_maintenance, last_history_sync }
struct ProcessState { running: HashMap<usize, JoinHandle<()>>, completed: HashSet<String> }
struct ConnectionState { event_tx: Option<EventTx>, acp: Option<AcpHandle> }
```

Wrapped in separate `Arc<Mutex<_>>` or combined under a single lock with field-level access methods.

**Rationale**: Reduces lock contention — a chat command doesn't need to lock scheduler timestamps.

### 6. Config-based binary paths

Resolve binaries via `PATH` lookup (`which` crate or `std::process::Command` PATH resolution) with fallback to env vars: `GLAB_PATH`, `SLACK_API_PATH`, `LINEAR_API_PATH`, `KIRO_CLI_PATH`.

**Rationale**: Hardcoded nix store hashes and absolute paths break on any environment change.

## Risks / Trade-offs

- [Risk] Large refactor touching all of backend → Mitigation: do it incrementally — extract modules one at a time, keeping tests green after each step
- [Risk] Splitting AppState may introduce deadlocks if multiple locks acquired in different orders → Mitigation: use a single outer struct with fine-grained inner mutexes, always lock in consistent order
- [Risk] Changing fetcher signatures is a wide diff → Mitigation: change one fetcher first as proof, then apply pattern to rest
- [Trade-off] Adding `&dyn CommandRunner` parameter adds a layer of indirection — acceptable cost for testability
