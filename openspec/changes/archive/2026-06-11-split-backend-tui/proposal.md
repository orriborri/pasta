## Why

The application currently runs fetchers, agent scheduling, and TUI rendering in a single binary with tightly coupled modules. This makes it impossible to run the backend (fetchers + agents) headlessly (e.g., as a service or on a remote machine) while interacting via TUI from elsewhere. It also couples the rendering loop to the scheduler tick rate and makes each part harder to develop and test independently.

## What Changes

- Extract backend logic (scheduler, fetchers, ACP client, data persistence) into a standalone daemon/library crate
- Extract TUI into a separate binary crate that connects to the backend
- Introduce IPC between backend and TUI (e.g., Unix socket or local TCP) so both can run as separate processes
- Backend runs headlessly, manages schedules, spawns agents, fetches feeds
- TUI connects to backend, renders state, and sends commands (run now, add schedule, chat prompt, etc.)

## Capabilities

### New Capabilities
- `backend-daemon`: Headless backend process that runs fetchers, scheduler, and agents independently of any UI
- `tui-client`: TUI binary that connects to the running backend and provides the interactive interface
- `backend-ipc`: IPC protocol between backend and TUI for state sync and commands

### Modified Capabilities

## Impact

- `Cargo.toml` restructured into a workspace with `crates/backend` and `crates/tui`
- `src/scheduler.rs`, `src/fetchers/`, `src/acp.rs`, `src/data.rs` move to backend crate
- `src/ui.rs`, `src/main.rs` (key handlers) move to TUI crate
- `src/vault.rs` shared as a library dependency (or in a `crates/common` crate)
- New IPC layer (message types, serialization, connection handling)
- Users need to start the backend separately (or TUI auto-starts it)
