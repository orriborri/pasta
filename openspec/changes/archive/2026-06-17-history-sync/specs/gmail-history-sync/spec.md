## ADDED Requirements

### Requirement: Sync relevant email threads to local files
The system SHALL fetch sent and received emails (excluding promotions, social, and automated notifications) and store as markdown files.

#### Scenario: First sync of emails
- **WHEN** history sync runs and no gmail history exists
- **THEN** all relevant emails (last 6 months) are fetched and stored in `.history/gmail/<YYYY-MM>/<subject-slug>.md`

#### Scenario: Incremental sync
- **WHEN** history sync runs and gmail files already exist
- **THEN** only threads newer than last sync are fetched and added

### Requirement: Group by month and thread
The system SHALL organize email files by month directory and one file per thread.

#### Scenario: Email thread with multiple messages
- **WHEN** a thread has multiple messages
- **THEN** all messages in the thread are stored in a single file with sender, date, and body for each

### Requirement: Exclude noise
The system SHALL exclude automated notifications (GitLab, calendar invites, promotions, social).

#### Scenario: GitLab notification email
- **WHEN** an email has the "Gitlab" label
- **THEN** it is excluded from history sync
