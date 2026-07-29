## ADDED Requirements

### Requirement: Sync state exposes the oldest cursor for retroactive fetching
The system SHALL expose the stored oldest timestamp per channel so fetchers can page retroactively and resume backfill across runs. The `cursors` table already persists `oldest_ts`; a reader SHALL return it alongside the existing newest-cursor reader.

#### Scenario: Oldest cursor read for backfill
- **WHEN** a fetcher's `fetch_backfill` runs for a channel with a stored `oldest_ts`
- **THEN** the sync state returns that timestamp
- **AND** the fetcher requests records older than it

#### Scenario: Oldest cursor absent
- **WHEN** no cursor row exists for a channel
- **THEN** the oldest-cursor reader returns none and the fetcher uses a default lookback floor

### Requirement: Backfill extends the oldest cursor monotonically
When a retroactive fetch completes, the system SHALL extend the channel's oldest cursor to the oldest record retrieved, using the existing `MIN(oldest_ts)` window-merge semantics so concurrent forward updates are not clobbered.

#### Scenario: Backfill page persisted
- **WHEN** `fetch_backfill` returns records older than the current oldest cursor
- **THEN** the window update lowers `oldest_ts` to the oldest returned record
- **AND** leaves `newest_ts` unchanged
