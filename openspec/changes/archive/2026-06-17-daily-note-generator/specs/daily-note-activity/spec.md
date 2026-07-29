## ADDED Requirements

### Requirement: Generate activity section from today's synced data
The system SHALL generate an "Activity Feed" section containing grouped highlights from Slack, Gmail, Linear, and code changes synced today.

#### Scenario: Sync completed with new activity
- **WHEN** `--daily` is run after a successful sync with new data
- **THEN** an activity section is appended to today's daily note grouped by source

#### Scenario: No new activity today
- **WHEN** no new data was synced today
- **THEN** the system skips daily note generation

### Requirement: Backlink to PARA documents
The system SHALL add `[[wikilinks]]` to relevant Projects, Areas, and Resources documents based on content similarity.

#### Scenario: Slack message relates to a project
- **WHEN** a Slack message about "deployment pipeline" is processed
- **THEN** the activity entry includes `[[Fix pipelines]]` if that project exists

#### Scenario: No matching PARA document
- **WHEN** an activity item has no similar PARA document above threshold
- **THEN** no backlink is added (item still shown without link)

### Requirement: Idempotent generation
The system SHALL replace the activity section if it already exists, not duplicate it.

#### Scenario: Running twice on same day
- **WHEN** `--daily` is run twice on the same day
- **THEN** the activity section is replaced with updated content, not appended twice

### Requirement: Preserve existing daily note content
The system SHALL not modify any content outside the activity section.

#### Scenario: Daily note has user notes
- **WHEN** the user has written in the Capture or Focus sections
- **THEN** those sections remain unchanged after activity generation

### Requirement: Create daily note if missing
The system SHALL create the daily note file if it doesn't exist, using minimal frontmatter.

#### Scenario: No daily note for today
- **WHEN** `--daily` runs and no file exists at `0. Inbox/Daily/YYYY-MM-DD.md`
- **THEN** the system creates it with frontmatter and the activity section

### Requirement: Suggest PARA placement for inbox notes
The system SHALL search the vector store for similar PARA documents and add a Related section with backlinks and placement suggestions to unprocessed inbox notes.

#### Scenario: Inbox note similar to a project
- **WHEN** an inbox note's content is similar to an existing project document
- **THEN** a `## Related` section is appended with `[[Project Name]]` and a placement suggestion

#### Scenario: Inbox note already processed
- **WHEN** an inbox note has `related: true` in frontmatter
- **THEN** it is skipped
