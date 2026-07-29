## ADDED Requirements

### Requirement: Display all sync stages with status indicators
The system SHALL display all sync stages (Slack, Gmail, Linear, Wiki, Repos, Indexing) on startup with status indicators: ○ (pending), ▸ (active), ✓ (done).

#### Scenario: Initial display shows all stages as pending
- **WHEN** the sync process starts
- **THEN** all stages are displayed with ○ indicator

#### Scenario: Active stage shows spinner
- **WHEN** a sync stage begins processing
- **THEN** that stage's indicator changes to ▸ with a description of current activity

#### Scenario: Completed stage shows checkmark with summary
- **WHEN** a sync stage completes
- **THEN** that stage's indicator changes to ✓ with item count (e.g., "63 channels  26,932 msgs")

### Requirement: Display memory usage on active stage
The system SHALL show current RSS memory usage on the currently active stage line.

#### Scenario: Memory shown during processing
- **WHEN** a stage is actively processing
- **THEN** the current RSS in MB is displayed on that line, updated every 10 seconds

### Requirement: Display progress bar with ETA during indexing
The system SHALL show a progress bar with percentage, files done/total, and ETA during the indexing stage.

#### Scenario: Indexing progress display
- **WHEN** the indexer pipeline is running
- **THEN** a progress bar is displayed showing files_done/total_files, percentage, and estimated time remaining

#### Scenario: ETA updates as indexing progresses
- **WHEN** more files are indexed
- **THEN** the ETA recalculates based on elapsed time and files remaining

### Requirement: Display final summary
The system SHALL display a summary line after sync completes showing total duration and peak memory usage.

#### Scenario: Sync completes successfully
- **WHEN** all stages complete without error
- **THEN** display "Done in Xm Ys · Peak memory: NMB"

#### Scenario: Sync completes with errors
- **WHEN** a stage encounters an error
- **THEN** display the error on that stage's line and continue to next stage

### Requirement: Verbose logs to file only
The system SHALL write all tracing output (debug, info, error) exclusively to `./logs/tracing.log`, not to stderr.

#### Scenario: Stderr shows only progress display
- **WHEN** the sync is running
- **THEN** stderr contains only the progress display, no tracing log lines

#### Scenario: Log file contains full tracing output
- **WHEN** the sync is running
- **THEN** `./logs/tracing.log` contains all structured tracing events with timestamps
