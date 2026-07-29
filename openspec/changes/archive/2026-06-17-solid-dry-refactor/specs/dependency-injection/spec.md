## ADDED Requirements

### Requirement: Fetchers accept CommandRunner parameter
All fetcher `fetch_raw()` functions SHALL accept a `&dyn CommandRunner` parameter instead of calling the global `run_cmd()` directly.

#### Scenario: GitLab fetcher uses injected runner
- **WHEN** `gitlab::fetch_raw(runner)` is called
- **THEN** it uses `runner.run()` for all external command invocations

#### Scenario: Mock runner enables testing
- **WHEN** a test provides a mock `CommandRunner` returning fixture JSON
- **THEN** `fetch_raw()` produces parsed data without network access

### Requirement: Binary paths are resolved from environment
External binary paths (glab, slack-api, linear-api, kiro-cli) SHALL be resolved via PATH lookup or environment variables. Hardcoded absolute paths MUST NOT exist in the source.

#### Scenario: Binary found in PATH
- **WHEN** `glab` is available in the system PATH
- **THEN** the fetcher uses the PATH-resolved binary

#### Scenario: Binary path overridden by env var
- **WHEN** `GLAB_PATH` environment variable is set
- **THEN** the fetcher uses that path instead of PATH lookup

#### Scenario: Binary not found
- **WHEN** a required binary is not in PATH and no env override is set
- **THEN** the fetcher returns an error with a descriptive message

### Requirement: Progress reporting via trait injection
`run_sync()` SHALL accept a progress reporter implementing the existing `StageProgress + IndexProgress` traits rather than constructing `SyncProgress` internally.

#### Scenario: CLI sync uses terminal progress
- **WHEN** `--sync-once` is run from the terminal
- **THEN** `run_sync` is called with a `SyncProgress` instance (terminal bars)

#### Scenario: Daemon sync uses silent progress
- **WHEN** the scheduler triggers a sync
- **THEN** `run_sync` is called with a no-op or logging-only progress implementation

### Requirement: Error observability
Functions that currently swallow errors with `.ok()` SHALL log the error via `tracing::error!` or `tracing::warn!` before discarding the result.

#### Scenario: Fetch failure is logged
- **WHEN** a fetcher command fails
- **THEN** a `tracing::warn!` is emitted with the command name and error

#### Scenario: Index failure is logged
- **WHEN** `index_new_files` returns an error during the fetch cycle
- **THEN** a `tracing::error!` is emitted with the error details
