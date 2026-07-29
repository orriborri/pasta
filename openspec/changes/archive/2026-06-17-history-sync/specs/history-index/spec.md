## ADDED Requirements

### Requirement: LanceDB semantic index of all synced content
The system SHALL maintain a LanceDB vector index of all history content, enabling semantic search across sources.

#### Scenario: Agent searches for a topic
- **WHEN** an agent queries "deployment strategy for EKS"
- **THEN** LanceDB returns the most relevant chunks from wiki, Slack, Linear, Gmail, and repos ranked by similarity

#### Scenario: Incremental index update
- **WHEN** new content is synced
- **THEN** only new/changed chunks are embedded and added to LanceDB (not full re-index)

### Requirement: Sync state tracking for incremental updates
The system SHALL track last sync timestamps per source in `_sync_state.json`.

#### Scenario: Interrupted sync resumes
- **WHEN** a sync is interrupted and restarted
- **THEN** it resumes from the last recorded state

### Requirement: Index wiki pages
The system SHALL sync and index the ReadPeak wiki markdown pages.

#### Scenario: Wiki page synced
- **WHEN** history sync runs
- **THEN** all wiki pages from `/home/orre/ReadPeak/wiki/` are copied to `.history/wiki/` and indexed in LanceDB

### Requirement: Index selected repo content
The system SHALL sync and index configured source files from key repositories.

#### Scenario: Repo files indexed
- **WHEN** history sync runs with repo configuration
- **THEN** matching files (README, docs, config, key source) are copied to `.history/repos/` and indexed

### Requirement: Frontmatter on all history files
Each history file SHALL have YAML frontmatter with source, type, participants, and last_updated.

#### Scenario: Agent searches by participant
- **WHEN** an agent greps for `participants:.*"Person Name"`
- **THEN** it finds all history files involving that person

### Requirement: Search CLI for agents
The system SHALL provide a search command that agents can call to query the knowledge base.

#### Scenario: Agent invokes search
- **WHEN** agent calls `pasta-search "topic" --source wiki,slack --limit 10`
- **THEN** it receives ranked chunks with source path, date, and content snippet
