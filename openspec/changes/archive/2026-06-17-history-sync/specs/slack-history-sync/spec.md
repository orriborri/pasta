## ADDED Requirements

### Requirement: Sync all DM conversations to local files
The system SHALL fetch all DM conversation history and store each as a markdown file named by the person.

#### Scenario: First sync of DMs
- **WHEN** history sync runs and no DM files exist
- **THEN** all DM conversations are fetched (last 6 months) and written to `.history/slack/dm/<person-name>.md`

#### Scenario: Incremental DM sync
- **WHEN** history sync runs and DM files already exist
- **THEN** only messages newer than the last sync timestamp are fetched and appended

### Requirement: Sync relevant channel history
The system SHALL fetch recent history from channels the user participates in.

#### Scenario: Channel sync
- **WHEN** history sync runs
- **THEN** messages from the last 90 days are fetched for joined channels and stored in `.history/slack/channels/<channel-name>.md`

### Requirement: Resolve user IDs to names
The system SHALL use display names (not user IDs) in all stored messages.

#### Scenario: Message with user ID
- **WHEN** a message has a raw user ID
- **THEN** it is resolved to the person's display name before writing to file

### Requirement: Respect rate limits
The system SHALL not exceed Slack API rate limits (Tier 3: ~50 requests/minute).

#### Scenario: Many conversations to fetch
- **WHEN** syncing requires more than 50 API calls
- **THEN** the system throttles requests with appropriate delays between calls
