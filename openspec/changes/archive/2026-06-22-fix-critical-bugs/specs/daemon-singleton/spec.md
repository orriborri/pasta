## ADDED Requirements

### Requirement: Single daemon instance enforcement
The system SHALL prevent multiple daemon instances from running concurrently using a file lock at `~/.kiro/pasta.lock`.

#### Scenario: First daemon starts
- **WHEN** `pasta-backend` starts and no other instance holds the lock
- **THEN** it acquires an exclusive lock on `~/.kiro/pasta.lock` and proceeds with startup

#### Scenario: Second daemon attempts to start
- **WHEN** `pasta-backend` starts and another instance already holds the lock
- **THEN** it prints "Another pasta-backend is already running" to stderr and exits with code 1

#### Scenario: Daemon crashes
- **WHEN** a running daemon process is killed or crashes
- **THEN** the kernel releases the `flock` automatically and a new instance can start

### Requirement: Lock file uses flock not PID
The lock mechanism SHALL use `flock(LOCK_EX | LOCK_NB)` rather than writing a PID to the file.

#### Scenario: Stale lock file exists from crash
- **WHEN** `~/.kiro/pasta.lock` exists but no process holds a lock on it
- **THEN** the new daemon acquires the lock successfully and starts normally
