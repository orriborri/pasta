# Agent Timeout

## Purpose

Lifecycle controls for spawned agent processes: enforce a configurable runtime timeout and prune stale agent logs during vault maintenance.

## Requirements

### Requirement: Agent process timeout
The system SHALL kill any spawned agent process that exceeds the configured timeout duration.

#### Scenario: Agent completes within timeout
- **WHEN** an agent finishes in less than `agent_timeout_minutes`
- **THEN** it exits normally and is marked as completed

#### Scenario: Agent exceeds timeout
- **WHEN** an agent process runs longer than `agent_timeout_minutes`
- **THEN** the process is killed with SIGKILL, a log entry "TIMEOUT — killed" is written, and an `AgentFinished` event is emitted

#### Scenario: Default timeout
- **WHEN** no `agent_timeout_minutes` is configured
- **THEN** the default timeout of 10 minutes is used

### Requirement: Log cleanup
The system SHALL delete agent log files older than 7 days during each vault-maintenance cycle.

#### Scenario: Old logs exist
- **WHEN** vault-maintenance runs and `logs/` contains `.log` files with mtime > 7 days
- **THEN** those files are deleted

#### Scenario: Recent logs preserved
- **WHEN** vault-maintenance runs and `logs/` contains `.log` files with mtime ≤ 7 days
- **THEN** those files are not deleted
