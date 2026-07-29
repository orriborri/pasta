## ADDED Requirements

### Requirement: Fetch unread DMs
The Slack fetcher SHALL retrieve recent messages from all open DM conversations.

#### Scenario: DMs with unread messages
- **WHEN** the fetcher runs
- **THEN** it lists DM conversations via `conversations.list types=im`, fetches recent messages from open ones, and includes them in the feed with sender name and message preview

### Requirement: Fetch unread channel messages
The Slack fetcher SHALL retrieve unread messages from channels the user is in.

#### Scenario: Channels with unread messages
- **WHEN** the fetcher runs
- **THEN** it lists channels via `conversations.list`, identifies those with `unread_count > 0`, fetches recent unread messages, and includes them in the feed grouped by channel

### Requirement: Resolve user display names
The Slack fetcher SHALL resolve Slack user IDs to display names in the feed output.

#### Scenario: Message from a user
- **WHEN** a message has a `user` field with a Slack user ID
- **THEN** the feed displays the user's real name or display name instead of the raw ID

### Requirement: Limit API calls
The Slack fetcher SHALL cap the number of conversations fetched to avoid rate limits.

#### Scenario: Many DM conversations
- **WHEN** there are more than 20 open DM conversations
- **THEN** only the 20 most recent are fetched
