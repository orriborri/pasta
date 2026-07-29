## ADDED Requirements

### Requirement: First sync fetches only last 7 days
The system SHALL fetch only the last 7 days of data on first run when no fetch boundaries exist in state.

#### Scenario: No existing state for any source
- **WHEN** a source syncs with no `newest_fetched` or `oldest_fetched`
- **THEN** it fetches data from `now - 7d` to `now` and stores both boundaries

### Requirement: Forward sync fetches only new data
The system SHALL fetch only data newer than `newest_fetched` on subsequent runs.

#### Scenario: Subsequent sync
- **WHEN** a source syncs with `newest_fetched` set
- **THEN** it queries only data after that boundary and updates `newest_fetched` to now

### Requirement: Backward sync extends by one week without overlap
The system SHALL fetch one additional week of older history per sync run, ending at the current `oldest_fetched` boundary.

#### Scenario: Backward expansion
- **WHEN** a source syncs with `oldest_fetched = "2026/06/06"`
- **THEN** it queries `oldest_fetched - 7d` to `oldest_fetched` and updates `oldest_fetched` to the new earlier date

### Requirement: No data is re-fetched
The system SHALL NOT query date ranges already covered by `newest_fetched` and `oldest_fetched`.

#### Scenario: Forward and backward never overlap
- **WHEN** both forward and backward queries complete
- **THEN** the updated boundaries are contiguous with no gap and no overlap with previously fetched ranges

### Requirement: Slack uses existing cursors with bounded fallback
The system SHALL keep Slack's per-channel cursor mechanism but cap the fallback to `oldest_fetched` instead of hardcoded 90/180 days.

#### Scenario: Channel with cursor
- **WHEN** a Slack channel has a cursor in state
- **THEN** it fetches only messages newer than that cursor (existing behavior)

#### Scenario: Channel without cursor
- **WHEN** a Slack channel has no cursor
- **THEN** it uses `oldest_fetched` as the oldest boundary, not 90/180 days ago

### Requirement: External commands have a 60s timeout
The system SHALL kill external commands that exceed 60 seconds and report the timeout.

#### Scenario: Command exceeds timeout
- **WHEN** an external command does not exit within 60 seconds
- **THEN** the process is killed and the error is logged with stderr output

### Requirement: Failed commands log stderr
The system SHALL capture and log stderr from external commands that exit with non-zero status.

#### Scenario: Command fails
- **WHEN** an external command exits with non-zero status
- **THEN** stderr content is logged via tracing at warn level
