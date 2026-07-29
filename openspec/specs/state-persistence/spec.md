# State Persistence

## Purpose

Concurrency-safe, corruption-resistant persistence of daemon sync state and related file writes, ensuring parallel tasks (fetchers, indexer) do not clobber each other's updates.

## Requirements

### Requirement: Atomic state file writes
The system SHALL use exclusive file locking (`flock`) when writing `_sync_state.json` to prevent concurrent write corruption.

#### Scenario: Two tasks save state concurrently
- **WHEN** the indexer writer task and fetch_cycle both attempt to save state simultaneously
- **THEN** the second writer blocks until the first completes, and both writes are fully persisted without data loss

#### Scenario: State read during write
- **WHEN** `load_state` is called while another task holds the write lock
- **THEN** it uses a shared lock and waits for the writer to finish before reading

### Requirement: Indexer does not overwrite its own hash updates
The system SHALL NOT save a pre-indexing state snapshot after `index_new_files` completes, since the indexer persists hashes internally during execution.

#### Scenario: fetch_cycle calls indexer
- **WHEN** `fetch_cycle::run` calls `index_new_files` which persists file hashes internally
- **THEN** no stale state snapshot is saved after the indexer returns

#### Scenario: CLI sync_once calls run_sync
- **WHEN** `run_sync` completes indexing and the caller saves state afterward
- **THEN** it reloads fresh state from disk (reflecting indexer's writes) before saving its own fields

### Requirement: Parallel sync state merge preserves concurrent updates
The system SHALL reload state from disk after parallel sync tasks complete, then apply source-specific field updates, rather than saving a clone taken before the parallel execution.

#### Scenario: Slack and Gmail sync complete with intervening indexer write
- **WHEN** `sync_slack` and `sync_gmail` run in parallel and the indexer writes hashes during their execution
- **THEN** the final state save preserves both the new sync cursors AND the indexer's file hashes

### Requirement: Async-compatible Slack rate limiting
The system SHALL use `tokio::time::sleep` instead of `std::thread::sleep` for Slack API rate limiting within async functions.

#### Scenario: Fetching paginated Slack history
- **WHEN** the sync_slack module pauses between API pages for rate limiting
- **THEN** it yields the async runtime (does not block the tokio worker thread)

### Requirement: Response-driven ACP initialization
The system SHALL wait for JSON-RPC responses from the ACP subprocess rather than using fixed-duration sleeps for initialization timing.

#### Scenario: ACP initialize handshake
- **WHEN** the backend sends `initialize` to kiro-cli via ACP
- **THEN** it reads stdout until a response with the matching request ID is received, with a 10-second timeout

#### Scenario: ACP initialization timeout
- **WHEN** the kiro-cli subprocess does not respond within 10 seconds
- **THEN** the system kills the subprocess and returns an error to the TUI

### Requirement: Daily note preserves content after activity feed
The system SHALL preserve any content that exists after the activity feed section when regenerating it.

#### Scenario: User adds notes below activity feed
- **WHEN** `generate_daily` runs and the daily note contains user content after `## 🔗 Activity Feed`
- **THEN** the user content is preserved in the regenerated file, appearing after the updated activity feed

#### Scenario: Activity feed followed by another heading
- **WHEN** the daily note has `## 🔗 Activity Feed` followed by `## My Notes`
- **THEN** only the content between those two headings is replaced; `## My Notes` and its content remain intact

### Requirement: Fetch cycle reports success conditionally
The system SHALL only mark a fetcher as completed if it returned non-empty data.

#### Scenario: GitLab API is unreachable
- **WHEN** the gitlab fetcher returns empty/error during a fetch cycle
- **THEN** `gitlab-fetcher` is NOT inserted into the completed set, and dependent agents are not triggered

#### Scenario: All fetchers succeed
- **WHEN** all fetchers return valid data
- **THEN** all fetcher names are inserted into the completed set and dependent agents trigger normally

### Requirement: Log cleanup uses absolute path
The system SHALL resolve the log directory path from the daemon's known location rather than using a relative `./logs` path.

#### Scenario: Daemon started from different working directory
- **WHEN** `cleanup_old_logs` runs and the daemon's CWD is not the pasta project root
- **THEN** it still correctly finds and cleans logs from the pasta project's `logs/` directory

### Requirement: Linear history sync uses configured binary path
The system SHALL use `resolve_binary("linear-api", ...)` in `history/sync_linear.rs` instead of a hardcoded path.

#### Scenario: linear-api binary configured in pasta.toml
- **WHEN** `[binaries] linear_api = "/custom/path/linear-api"` is set in config
- **THEN** history sync uses that configured path

### Requirement: Stale task flag is reversible
The system SHALL remove the `stale: true` frontmatter field when a task is updated (status change, due date added, or manual edit detected via mtime).

#### Scenario: User adds a due date to a stale task
- **WHEN** vault maintenance runs and a task marked `stale: true` now has a `due:` field
- **THEN** the `stale: true` line is removed from the task's frontmatter

### Requirement: Git log parsing handles multi-paragraph commit messages
The system SHALL use null-byte/SOH record separators for git log parsing instead of splitting on double newlines.

#### Scenario: Commit with multi-paragraph body
- **WHEN** a git commit has a body containing blank lines (e.g., bullet lists separated by empty lines)
- **THEN** the entire body is captured in a single LogEntry, not split across multiple entries

#### Scenario: Commit with no body
- **WHEN** a git commit has only a subject line and no body
- **THEN** the entry is parsed correctly with an empty body field

### Requirement: Vault link repair uses document content, not frontmatter
The system SHALL strip YAML frontmatter before constructing the search query for finding related documents in `repair_links`.

#### Scenario: Document with standard frontmatter
- **WHEN** `repair_links` processes a vault document that starts with `---\nsource: vault\n...---`
- **THEN** the search query is built from the content after the frontmatter closing `---`

#### Scenario: Document with only frontmatter and short content
- **WHEN** the content after frontmatter is fewer than 50 characters
- **THEN** the document title (filename stem) is used as the search query instead

### Requirement: Triage patterns persisted as JSON
The system SHALL store triage patterns in `0. Inbox/triage-patterns.json` with fields: source, keywords, column, hits, misses, last_updated.

#### Scenario: First pattern created
- **WHEN** a source+keyword→column combination is seen 3 times consistently
- **THEN** a new entry is added to `triage-patterns.json`

#### Scenario: Patterns file read during daily note generation
- **WHEN** the daily-writer needs to suggest columns for untagged tasks
- **THEN** it reads `triage-patterns.json` and matches against untagged task titles and sources

### Requirement: Initiative embeddings available for routing queries
The system SHALL ensure initiative description content is indexed in LanceDB so that vector similarity search can match tasks against initiatives.

#### Scenario: New initiative added to Roadmap
- **WHEN** a new file `Roadmap/ML Platform.md` is created
- **AND** the next history-sync runs
- **THEN** its content is embedded and searchable in LanceDB for routing comparisons
