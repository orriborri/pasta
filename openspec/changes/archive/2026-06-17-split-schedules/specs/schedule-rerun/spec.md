## ADDED Requirements

### Requirement: Re-run native schedule from TUI
The user SHALL be able to manually trigger any native schedule from the TUI by pressing `r` on the selected row.

#### Scenario: Re-run idle native schedule
- **WHEN** user selects a native schedule row and presses `r`
- **AND** that schedule is not currently running
- **THEN** system triggers the schedule immediately, resets its timer, and shows flash "↻ {name} triggered"

#### Scenario: Re-run already-running native schedule
- **WHEN** user selects a native schedule row and presses `r`
- **AND** that schedule is currently running
- **THEN** system shows flash "{name} already running" and does not queue a second run

### Requirement: Re-run agentic schedule from TUI
The user SHALL be able to manually trigger any agentic schedule from the TUI by pressing `r` on the selected row.

#### Scenario: Re-run idle agentic schedule
- **WHEN** user selects an agentic schedule row and presses `r`
- **AND** that agent is not currently running
- **THEN** system spawns the agent immediately, resets its timer, and shows flash "↻ {agent} triggered"

#### Scenario: Re-run already-running agentic schedule
- **WHEN** user selects an agentic schedule row and presses `r`
- **AND** that agent is currently running
- **THEN** system shows flash "{agent} already running" and does not spawn a duplicate

### Requirement: Two-section TUI display
The schedule tab SHALL display native schedules and agentic schedules in separate visual sections with distinct column headers.

#### Scenario: Schedule tab rendered
- **WHEN** user views the schedule tab
- **THEN** native schedules appear in an upper section with columns: Name, Freq, Next, Last Run
- **AND** agentic schedules appear in a lower section with columns: Agent, Trigger, Next, Last Run, Prompt

#### Scenario: Cursor spans both sections
- **WHEN** user navigates with arrow keys in the schedule tab
- **THEN** cursor moves through both native and agentic rows seamlessly, crossing the section divider
