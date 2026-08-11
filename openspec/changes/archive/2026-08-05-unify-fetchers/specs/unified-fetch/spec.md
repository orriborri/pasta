## ADDED Requirements

### Requirement: Single fetch pass produces both feeds and records
The system SHALL use kb-fetchers as the sole data source for external services (Slack, Gmail, Linear, GitLab, Calendar), producing `Vec<Record>` that drives both TUI feed generation and search indexing.

#### Scenario: Fetch cycle runs
- **WHEN** the fetch cycle timer fires
- **THEN** kb-fetchers run once per source, returning `Vec<Record>`
- **AND** `.feeds/` markdown files are generated from those records
- **AND** the ingestion pipeline indexes the same records into search stores

#### Scenario: API rate-limited during fetch
- **WHEN** the Slack API returns a rate-limit error
- **THEN** the kb-fetcher retries with backoff (existing kb-fetcher behavior)
- **AND** the fetch cycle waits for completion before generating feeds

### Requirement: Feed generation runs before indexing
The system SHALL generate `.feeds/` markdown files from fetched records BEFORE running the ingestion pipeline, so TUI feeds update even if indexing fails.

#### Scenario: Indexing fails after fetch
- **WHEN** kb-fetchers successfully return records
- **AND** the embedding service is unreachable
- **THEN** `.feeds/` files are still updated with current data
- **AND** the indexing failure is logged as a warning

### Requirement: Feed markdown format preserves current structure
The system SHALL produce `.feeds/` markdown in the same format as the current backend fetchers (frontmatter with source/fetched, sections with unread items), derived from `Record` fields.

#### Scenario: Slack feed generated from records
- **WHEN** Slack fetch returns DM and channel records
- **THEN** `.feeds/slack.md` contains "## Unread DMs" and "## Unread Channels" sections
- **AND** each entry shows author, message preview, and timestamp

### Requirement: No separate kb-sync schedule exists
The system SHALL NOT run `kb-sync` as a standalone native schedule. The indexing pipeline runs inline within the unified fetch cycle.

#### Scenario: Scheduler tick with no kb-sync entry
- **WHEN** the scheduler iterates native schedules
- **THEN** there is no `kb-sync` entry in REGISTRY or KNOWN_NATIVE_SCHEDULES
- **AND** the `kb` CLI `sync` command still works independently for manual runs

### Requirement: Completed set updated from fetch results
The system SHALL update the `completed` set (used for agentic schedule triggers) based on whether kb-fetcher records contain data for each source.

#### Scenario: Slack fetcher returns new messages
- **WHEN** the Slack kb-fetcher returns non-empty records
- **THEN** `"slack-fetcher"` is inserted into the completed set
- **AND** agents depending on `"slack-fetcher"` are triggered
