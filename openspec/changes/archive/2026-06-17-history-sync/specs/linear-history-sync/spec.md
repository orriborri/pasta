## ADDED Requirements

### Requirement: Sync all relevant Linear issues with comments
The system SHALL fetch all issues the user created, was assigned, or commented on, including full comment threads.

#### Scenario: First sync
- **WHEN** history sync runs and no linear history exists
- **THEN** all relevant issues are fetched with comments and stored in `.history/linear/<identifier>.md`

#### Scenario: Incremental sync
- **WHEN** history sync runs and linear files exist
- **THEN** only issues updated since last sync are re-fetched and overwritten

### Requirement: Include issue metadata
Each issue file SHALL include identifier, title, status, assignee, project, priority, and all comments with timestamps and authors.

#### Scenario: Issue with discussion
- **WHEN** an issue has 5 comments
- **THEN** the file contains the issue description followed by all 5 comments with author and date
