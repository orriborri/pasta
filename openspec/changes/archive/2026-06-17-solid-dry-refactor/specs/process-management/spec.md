## ADDED Requirements

### Requirement: Agent processes tracked via JoinHandle
Agent child processes SHALL be tracked using `tokio::task::JoinHandle` wrapping a `tokio::process::Child` instead of raw PID integers. The `unsafe` `libc::waitpid` call MUST be removed.

#### Scenario: Agent spawned and tracked
- **WHEN** `spawn_agent()` is called
- **THEN** it returns a `JoinHandle<ExitStatus>` that the scheduler stores by index

#### Scenario: Agent completion detected
- **WHEN** an agent's JoinHandle resolves
- **THEN** the scheduler marks it as completed and emits `AgentFinished` event

#### Scenario: No unsafe code in process tracking
- **WHEN** the codebase is compiled
- **THEN** no `unsafe` blocks exist in the process management module

### Requirement: AppState split into focused sub-states
`AppState` SHALL be decomposed into separate structs with distinct lock scopes: scheduling timestamps, process tracking, and connection state.

#### Scenario: Chat command does not lock scheduler state
- **WHEN** a `ChatPrompt` command is handled
- **THEN** only the connection/ACP state lock is acquired, not the scheduler timestamps

#### Scenario: Scheduler tick does not lock connection state
- **WHEN** the scheduler evaluates schedules
- **THEN** only the process and scheduler state locks are acquired

### Requirement: Graceful agent termination
When the backend shuts down, all running agent processes SHALL be signaled to terminate. Outstanding JoinHandles MUST be awaited with a timeout.

#### Scenario: Backend shutdown with running agents
- **WHEN** the backend receives a shutdown signal
- **THEN** it sends SIGTERM to all running agent child processes and awaits them for up to 5 seconds before force-killing
