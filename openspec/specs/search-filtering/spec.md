# Search Filtering

## Purpose

Apply source, participant, and date filters inside the vector search query (pre-limit) so result counts are honored, consistently across the MCP and backend search paths.

## Requirements

### Requirement: Vector search applies filters before limit
The system SHALL apply source, participant, and date filters as part of the vector search query (pre-limit) rather than filtering results after retrieval.

#### Scenario: Search with source filter and limit of 10
- **WHEN** a user searches with `source=slack` and `limit=10`
- **THEN** all 10 returned results have `source=slack` (not fewer due to post-filtering)

#### Scenario: Search with after-date filter
- **WHEN** a user searches with `after=2026-01-01` and `limit=5`
- **THEN** all 5 results have dates on or after 2026-01-01

### Requirement: MCP and backend search use consistent filtering
Both `crates/mcp/src/main.rs` and `crates/backend/src/history/indexer.rs` search functions SHALL use the same pre-limit filtering approach.

#### Scenario: MCP search matches backend search
- **WHEN** the same query with filters is executed via MCP tool and via backend Command::Search
- **THEN** both return identical results
