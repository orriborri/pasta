## ADDED Requirements

### Requirement: Backend runs as a headless daemon
The backend SHALL run without a terminal attached, managing all scheduler ticks, fetcher execution, and agent spawning independently of any UI.

#### Scenario: Start daemon directly
- **WHEN** user runs `pasta-backend` (or backend is spawned by TUI)
- **THEN** backend starts, binds to `~/.kiro/pasta.sock`, and begins scheduler loop

#### Scenario: Backend continues after TUI disconnect
- **WHEN** TUI disconnects from the socket
- **THEN** backend continues running all scheduled agents and fetchers without interruption

### Requirement: Backend owns ACP agent lifecycle
The backend SHALL spawn and manage ACP client processes, relaying chat events to connected TUI clients.

#### Scenario: Chat session started via IPC
- **WHEN** backend receives a `chat_start` command over IPC
- **THEN** backend spawns an ACP client process and begins relaying `ChatEvent`s back as IPC events

#### Scenario: TUI disconnects during active chat
- **WHEN** TUI disconnects while an ACP agent is running
- **THEN** backend keeps the ACP process alive; TUI can reconnect and resume receiving events

### Requirement: Backend exposes state over IPC
The backend SHALL push its current state (schedules, running agents, feeds, tasks) to connected clients on connect and on change.

#### Scenario: Client connects
- **WHEN** a TUI client connects to the socket
- **THEN** backend sends a full state snapshot immediately

#### Scenario: State changes
- **WHEN** a schedule runs, a fetch completes, or a task changes
- **THEN** backend pushes an incremental event to connected clients
