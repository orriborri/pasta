## ADDED Requirements

### Requirement: TUI connects to backend over IPC
The TUI SHALL connect to the backend daemon's Unix socket and render state received from it.

#### Scenario: Normal startup
- **WHEN** user runs `pasta` (TUI binary)
- **THEN** TUI connects to `~/.kiro/pasta.sock` and renders the current state

#### Scenario: Backend not running
- **WHEN** TUI starts and no backend socket exists
- **THEN** TUI spawns the backend daemon, waits for socket availability, then connects

### Requirement: TUI sends commands to backend
The TUI SHALL send user actions as commands over IPC rather than executing them directly.

#### Scenario: User triggers schedule run
- **WHEN** user presses Enter on a schedule entry
- **THEN** TUI sends a `run_now` command to backend; backend spawns the agent

#### Scenario: User adds a schedule
- **WHEN** user completes the add-schedule flow
- **THEN** TUI sends an `add_schedule` command to backend

#### Scenario: User sends chat message
- **WHEN** user types a message in chat mode and presses Enter
- **THEN** TUI sends a `chat_prompt` command to backend

### Requirement: TUI renders backend events
The TUI SHALL update its display in response to events pushed by the backend.

#### Scenario: Agent finishes
- **WHEN** backend sends an `agent_finished` event
- **THEN** TUI updates running indicators and refreshes state

#### Scenario: Chat streaming
- **WHEN** backend sends `chat_text` events
- **THEN** TUI appends text to the chat view in real-time

### Requirement: TUI handles disconnection gracefully
The TUI SHALL display a connection status and attempt reconnection if the backend becomes unreachable.

#### Scenario: Backend crashes
- **WHEN** the IPC connection drops unexpectedly
- **THEN** TUI shows "Disconnected" status and attempts to reconnect every 2 seconds
