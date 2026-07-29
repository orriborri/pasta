## ADDED Requirements

### Requirement: Main entrypoint is CLI-only
The `main.rs` file SHALL contain only CLI argument parsing and delegation to the appropriate entrypoint function. It MUST NOT contain server logic, scheduling, or command handling.

#### Scenario: Daemon mode delegates to server
- **WHEN** pasta-backend is started without CLI flags
- **THEN** main.rs calls `server::run()` which owns the accept loop

#### Scenario: CLI subcommands delegate to dedicated functions
- **WHEN** pasta-backend is started with `--sync-once`, `--reindex`, `--backfill`, `--organize`, or `--daily`
- **THEN** main.rs calls a single function in the appropriate module without inlining the logic

### Requirement: Scheduler is a separate module
The scheduler tick loop SHALL live in `scheduler.rs` with a public `async fn run(state)` entry point. It MUST own fetch cycle timing, agent schedule evaluation, and vault maintenance timing.

#### Scenario: Scheduler runs fetch cycle
- **WHEN** the native fetch interval has elapsed
- **THEN** scheduler calls the shared `run_fetch_cycle()` function

#### Scenario: Scheduler evaluates agent schedules
- **WHEN** a schedule's interval has elapsed or dependencies are met
- **THEN** scheduler spawns the agent via the process management module

### Requirement: Server module handles IPC
The `server.rs` module SHALL own `UnixListener` binding, client accept loop, and per-client read/write split. It MUST delegate command processing to `commands.rs`.

#### Scenario: Client connects and receives state
- **WHEN** a TUI client connects to the Unix socket
- **THEN** server sends the initial state snapshot and begins forwarding events

### Requirement: Single fetch cycle function
There SHALL be exactly one `async fn run_fetch_cycle()` that performs: fetch raw data, write feeds, write history records, index new files, and notify completion. Both the scheduler and `Command::ForceFetch` MUST call this same function.

#### Scenario: ForceFetch uses shared function
- **WHEN** a `ForceFetch` command is received
- **THEN** it calls the same `run_fetch_cycle()` as the scheduler, producing identical side effects

### Requirement: ACP protocol is isolated
The `acp.rs` module SHALL encapsulate ACP child spawning, JSON-RPC initialization, prompt sending, and notification parsing. No ACP protocol details SHALL leak into other modules.

#### Scenario: Chat command delegates to ACP module
- **WHEN** a `ChatStart` command is received
- **THEN** server delegates to `acp::spawn()` which returns an opaque handle

### Requirement: Commands module handles dispatch
The `commands.rs` module SHALL contain `handle_command()` and all match arms. Each arm MUST delegate to the appropriate module rather than containing inline logic.

#### Scenario: Command dispatch is thin
- **WHEN** any `Command` variant is received
- **THEN** the match arm calls a function in the appropriate module (scheduler, acp, or state)
