## ADDED Requirements

### Requirement: Native schedules appear in Schedule tab
The TUI SHALL display native/system schedules (fetchers, vault maintenance) in the Schedule tab alongside user-defined schedules.

#### Scenario: Schedule tab shows native fetchers
- **WHEN** user views the Schedule tab
- **THEN** native schedules (gitlab, linear, slack, gmail, calendar, vault-maintenance) are visible with their interval and last-run time

#### Scenario: Native schedules are visually distinct
- **WHEN** native schedules are rendered in the Schedule tab
- **THEN** they are displayed with a "system" label or dimmed style to distinguish from user-defined schedules

### Requirement: Native schedules are read-only
The TUI SHALL NOT allow users to delete, edit, or manually trigger native schedules from the Schedule tab (existing 'f' key for force-fetch remains separate).

#### Scenario: User cannot delete native schedule
- **WHEN** user selects a native schedule row and presses 'd'
- **THEN** nothing happens (delete action is skipped for native rows)

#### Scenario: User cannot run native schedule manually via Enter
- **WHEN** user selects a native schedule row and presses Enter
- **THEN** nothing happens (run-now action is skipped for native rows)

### Requirement: Backend exposes native schedule timing in state
The backend SHALL include native schedule information (name, interval, last run) in the State event sent to the TUI.

#### Scenario: State snapshot includes native schedules
- **WHEN** TUI connects and receives state
- **THEN** the state contains a list of native schedules with their current timing information
