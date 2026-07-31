## ADDED Requirements

### Requirement: All task files are visible regardless of depth
The system SHALL include every task file under the task directory, at any nesting depth, in the task set returned to clients.

#### Scenario: Task routed into an initiative subfolder
- **WHEN** vault maintenance moves `Tasks/kb-abc.md` to `Tasks/Ad Delivery/kb-abc.md`
- **THEN** the task still appears in the TUI task list and in the daemon's state snapshot

#### Scenario: Task set counted before and after routing
- **WHEN** the routing pass moves N tasks into initiative subfolders
- **THEN** the number of tasks in the state snapshot is unchanged

### Requirement: Task mutation resolves the file by its real path
The system SHALL mutate a task using the path it was loaded from, rather than reconstructing `<task directory>/<file name>`.

#### Scenario: Closing a task inside a subfolder
- **WHEN** the user closes a task that lives in `Tasks/Monitoring/fix-alerts.md`
- **THEN** that file's `status` becomes `done` and no file is created at `Tasks/fix-alerts.md`

#### Scenario: Approving a pending task inside a subfolder
- **WHEN** the user approves a pending task stored in a subfolder
- **THEN** the subfolder file's status changes from `pending` to `open`

### Requirement: One task enumeration is shared by all consumers
The system SHALL enumerate task files through a single function used by the state snapshot, the Trello sync, and vault maintenance, so no consumer sees a different task set.

#### Scenario: Trello sync and TUI disagree
- **WHEN** a task exists in a subfolder and both the Trello sync and the state snapshot run
- **THEN** both observe the same task, and the user can act on it from either surface
