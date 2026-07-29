## ADDED Requirements

### Requirement: IPC uses Unix domain socket with JSON-lines protocol
The system SHALL use a Unix domain socket at `~/.kiro/pasta.sock` with newline-delimited JSON messages.

#### Scenario: Message framing
- **WHEN** either side sends a message
- **THEN** it is serialized as a single JSON object followed by a newline (`\n`)

#### Scenario: Socket location
- **WHEN** backend starts
- **THEN** it binds to `~/.kiro/pasta.sock`, removing any stale socket file first

### Requirement: Command messages (TUI → Backend)
The protocol SHALL define command messages that the TUI sends to the backend to trigger actions.

#### Scenario: Run schedule now
- **WHEN** TUI sends `{"cmd": "run_now", "index": 0}`
- **THEN** backend spawns the agent for schedule at index 0

#### Scenario: Add schedule
- **WHEN** TUI sends `{"cmd": "add_schedule", "agent": "...", "prompt": "...", "interval_minutes": 60}`
- **THEN** backend adds the schedule and persists it

#### Scenario: Remove schedule
- **WHEN** TUI sends `{"cmd": "remove_schedule", "index": 2}`
- **THEN** backend removes the schedule at index 2

#### Scenario: Force fetch
- **WHEN** TUI sends `{"cmd": "force_fetch"}`
- **THEN** backend triggers an immediate native fetch cycle

#### Scenario: Chat prompt
- **WHEN** TUI sends `{"cmd": "chat_prompt", "text": "..."}`
- **THEN** backend relays the prompt to the ACP client

#### Scenario: Chat start
- **WHEN** TUI sends `{"cmd": "chat_start", "agent": "weekly-reviewer"}`
- **THEN** backend spawns an ACP client for the specified agent

#### Scenario: Chat stop
- **WHEN** TUI sends `{"cmd": "chat_stop"}`
- **THEN** backend kills the active ACP client process

### Requirement: Event messages (Backend → TUI)
The protocol SHALL define event messages that the backend pushes to the TUI.

#### Scenario: Full state sync on connect
- **WHEN** TUI connects
- **THEN** backend sends `{"evt": "state", "schedules": [...], "running": [...], "feeds": [...], "tasks": [...]}`

#### Scenario: Agent started event
- **WHEN** backend spawns an agent
- **THEN** it sends `{"evt": "agent_started", "index": N, "agent": "..."}`

#### Scenario: Agent finished event
- **WHEN** a running agent process exits
- **THEN** backend sends `{"evt": "agent_finished", "index": N, "agent": "..."}`

#### Scenario: Fetch complete event
- **WHEN** a native fetch cycle completes
- **THEN** backend sends `{"evt": "fetch_complete"}`

#### Scenario: Chat event relay
- **WHEN** ACP client produces a ChatEvent
- **THEN** backend sends `{"evt": "chat", "type": "text|tool_call|turn_end|error", "payload": {...}}`

#### Scenario: Schedule changed event
- **WHEN** a schedule is added or removed
- **THEN** backend sends `{"evt": "schedules_changed", "schedules": [...]}`
