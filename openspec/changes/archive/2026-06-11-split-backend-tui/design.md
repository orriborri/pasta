## Context

`pasta` is a single-binary Rust TUI application that combines:
- A scheduler that ticks every ~6s, running fetchers and spawning kiro-cli agents
- Native fetchers (gitlab, linear, slack, gmail, calendar) that write to `.feeds/`
- An ACP client for interactive chat with agents
- A ratatui-based TUI with tabs for tasks, schedules, people, feeds, stats
- Vault operations (reading/writing Obsidian markdown files)

All of this runs in one tokio runtime inside a single process. The TUI event loop polls at 100ms and the scheduler ticks at 60 iterations (~6s). State is shared via direct struct access and `Arc<Mutex<>>`.

## Goals / Non-Goals

**Goals:**
- Backend runs independently as a daemon (headless, no terminal required)
- TUI connects to backend over IPC and can be started/stopped without affecting backend
- State sync: TUI always reflects current backend state (schedules, running agents, feeds)
- Commands: TUI can trigger actions on backend (run now, add/remove schedule, force fetch)
- Chat passthrough: TUI relays ACP chat to backend which owns the agent process
- Zero-downtime reconnect: TUI can disconnect and reconnect without losing backend state

**Non-Goals:**
- Remote/network access (stay local-only for now, Unix socket)
- Multiple simultaneous TUI clients
- Changing the vault or fetcher logic itself
- Web UI or API server

## Decisions

### 1. Workspace layout with three crates

```
Cargo.toml (workspace)
crates/
  backend/    — daemon binary + library
  tui/        — TUI binary
  common/     — shared types (Schedule, Task, Feed, IPC messages)
```

**Rationale**: Keeps shared types in one place. Backend exposes a library for tests. TUI depends on common only (not backend internals).

**Alternative considered**: Two crates with types duplicated via serde — rejected due to drift risk.

### 2. Unix domain socket for IPC

Backend listens on `~/.kiro/pasta.sock`. TUI connects on startup. Protocol: newline-delimited JSON messages (simple, debuggable, no extra deps).

**Rationale**: Unix sockets are zero-latency local IPC, auto-cleanup on crash via filesystem. JSON-lines are trivial to debug with `socat`.

**Alternative considered**: gRPC — overkill for local single-client. Shared memory — complex synchronization.

### 3. Message protocol

Two message types:
- **Command** (TUI → Backend): `{ "cmd": "run_now", "payload": {...} }`
- **Event** (Backend → TUI): `{ "evt": "state_update", "payload": {...} }` 

Backend pushes full state snapshot on connect, then incremental events (schedule changed, agent started/finished, fetch complete, chat event).

**Rationale**: Simple request/push model. No request-response correlation needed since commands are fire-and-forget with state convergence via events.

### 4. Backend auto-start from TUI

TUI checks if socket exists and backend is alive. If not, spawns backend as a detached daemon before connecting.

**Rationale**: Seamless UX — user just runs `pasta` (the TUI) and everything works. Power users can run backend separately.

### 5. ACP chat ownership

Backend owns the ACP child process. TUI sends chat prompts as commands; backend relays ChatEvents back as IPC events.

**Rationale**: Backend already manages process lifecycle (scheduler spawns agents). Keeping chat in backend means TUI disconnect doesn't kill the agent mid-conversation.

## Risks / Trade-offs

- **Complexity increase** → Mitigated by keeping protocol minimal (JSON-lines, ~10 message types)
- **Stale TUI state** → Backend pushes events; TUI re-syncs full state on reconnect
- **Socket cleanup on crash** → Use `SO_PASSCRED` or PID file to detect stale sockets
- **Debug difficulty** → JSON-lines protocol is human-readable; add `--verbose` flag to log all IPC
- **Chat latency** → Negligible since Unix socket adds <1ms; streaming chunks flow the same way
