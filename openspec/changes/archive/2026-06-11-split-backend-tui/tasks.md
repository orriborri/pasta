## 1. Workspace Setup

- [x] 1.1 Convert to Cargo workspace with `crates/common`, `crates/backend`, `crates/tui`
- [x] 1.2 Move shared types (`Schedule`, `RunEntry`, `Task`, `Feed`, etc.) into `crates/common`
- [x] 1.3 Move `vault.rs` into `crates/common` (shared read/write ops)

## 2. IPC Protocol (crates/common)

- [x] 2.1 Define `Command` enum (RunNow, AddSchedule, RemoveSchedule, ForceFetch, ChatStart, ChatPrompt, ChatStop)
- [x] 2.2 Define `Event` enum (State, AgentStarted, AgentFinished, FetchComplete, Chat, SchedulesChanged)
- [x] 2.3 Implement JSON-lines serialize/deserialize for Command and Event
- [x] 2.4 Create socket path helper (`~/.kiro/pasta.sock`)

## 3. Backend Daemon (crates/backend)

- [x] 3.1 Create backend `main.rs` — bind Unix socket, start scheduler loop
- [x] 3.2 Move `scheduler.rs` logic into backend (tick loop, spawn agents, cleanup)
- [x] 3.3 Move `fetchers/` into backend
- [x] 3.4 Move `acp.rs` into backend
- [x] 3.5 Implement IPC server — accept connection, send state snapshot, push events
- [x] 3.6 Implement command handler — dispatch incoming commands to scheduler/acp/fetchers
- [x] 3.7 Handle stale socket cleanup on startup (remove dead socket file)

## 4. TUI Client (crates/tui)

- [x] 4.1 Create TUI `main.rs` — connect to socket, enter ratatui loop
- [x] 4.2 Move `ui.rs` rendering into TUI crate
- [x] 4.3 Replace direct state mutation with IPC event handling (update App from events)
- [x] 4.4 Replace direct action calls with IPC command sending
- [x] 4.5 Implement auto-start: spawn backend if socket not available
- [x] 4.6 Implement reconnection logic (retry every 2s, show "Disconnected" status)
- [x] 4.7 Wire chat mode to send ChatStart/ChatPrompt/ChatStop commands

## 5. Integration & Cleanup

- [x] 5.1 Remove old single-binary `src/main.rs` and flatten
- [x] 5.2 Verify `cargo build --workspace` compiles both binaries
- [x] 5.3 Test: start backend, connect TUI, trigger schedule, verify events flow
- [x] 5.4 Test: kill TUI, verify backend continues, reconnect TUI gets fresh state
