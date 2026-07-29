## ADDED Requirements

### Requirement: Native schedules configured via TOML
The system SHALL load native schedule configuration from `~/.kiro/native-schedules.toml` at daemon startup.

#### Scenario: Config file exists
- **WHEN** daemon starts and `~/.kiro/native-schedules.toml` exists
- **THEN** native schedule intervals and enabled state are loaded from the file

#### Scenario: Config file missing
- **WHEN** daemon starts and `~/.kiro/native-schedules.toml` does not exist
- **THEN** system generates a default config file with all schedules enabled at default intervals

#### Scenario: Unknown schedule name in config
- **WHEN** config contains a name not in the native schedule registry
- **THEN** system logs a warning and skips the unknown entry

### Requirement: Individual native schedule enable/disable
Each native schedule entry SHALL have an `enabled` boolean field. Disabled schedules MUST NOT run automatically.

#### Scenario: Schedule disabled
- **WHEN** a native schedule has `enabled = false`
- **THEN** the scheduler skips it during automatic evaluation

#### Scenario: Schedule enabled
- **WHEN** a native schedule has `enabled = true` and its interval has elapsed
- **THEN** the scheduler fires the schedule

### Requirement: Configurable intervals
Each native schedule entry SHALL have an `interval_minutes` field controlling how often it runs.

#### Scenario: Custom interval
- **WHEN** `gmail` has `interval_minutes = 30`
- **THEN** the gmail fetcher runs every 30 minutes instead of the default 60
