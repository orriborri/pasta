## ADDED Requirements

### Requirement: Triage patterns persisted as JSON
The system SHALL store triage patterns in `0. Inbox/triage-patterns.json` with fields: source, keywords, column, hits, misses, last_updated.

#### Scenario: First pattern created
- **WHEN** a source+keyword→column combination is seen 3 times consistently
- **THEN** a new entry is added to `triage-patterns.json`

#### Scenario: Patterns file read during daily note generation
- **WHEN** the daily-writer needs to suggest columns for untagged tasks
- **THEN** it reads `triage-patterns.json` and matches against untagged task titles and sources
