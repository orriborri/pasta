# Calendar Sync

## Purpose

Fetch calendar events via the `gog` CLI and store them as universal-schema Records, with attendees, organizer, and event timing preserved for search and context.

## Requirements

### Requirement: Calendar events fetched via gog CLI
The system SHALL fetch calendar events using `gog calendar events --json --from <cursor> --all` and convert them to Records.

#### Scenario: First sync
- **WHEN** `kb sync --source calendar` runs with no prior cursor
- **THEN** events from the last 30 days are fetched from all calendars

#### Scenario: Incremental sync
- **WHEN** `kb sync --source calendar` runs with an existing cursor
- **THEN** only events newer than the cursor are fetched

### Requirement: Events stored with attendees and time
Each calendar event SHALL be stored as a Record with attendees as participants, organizer as author, and event start time as created_at.

#### Scenario: Meeting with attendees
- **WHEN** a meeting "Auth Migration Discussion" has 4 attendees
- **THEN** the Record has all 4 emails in participants, the organizer as author, and the meeting description as content

#### Scenario: All-day event
- **WHEN** an all-day event has `start.date` instead of `start.dateTime`
- **THEN** the created_at is parsed as midnight UTC on that date

### Requirement: Cancelled events excluded
The system SHALL skip events with status "cancelled".

#### Scenario: Cancelled meeting
- **WHEN** a fetched event has `status: "cancelled"`
- **THEN** it is not stored as a Record
