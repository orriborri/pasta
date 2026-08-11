# Search Filtering

## Purpose

Apply source, participant, and date filters around the knowledge-base hybrid
search so requested result counts are honored, through a single search entry
point (`crates/backend/src/kb_search.rs`).

## Requirements

### Requirement: Over-fetch then filter to honor the limit
`hybrid_search` takes no filter arguments, so the system SHALL over-fetch and
then apply source, participant, and date filters in
`crates/backend/src/kb_search.rs`, taking up to `limit` results after filtering.

#### Scenario: Search with source filter and limit of 10
- **WHEN** a user searches with `source=slack` and `limit=10`
- **THEN** `kb_search` over-fetches (limit × 3), filters to `source=slack`, and returns up to 10 matching results

#### Scenario: Search with after-date filter
- **WHEN** a user searches with `after=2026-01-01` and `limit=5`
- **THEN** results are filtered to dates on or after 2026-01-01 before taking up to 5

### Requirement: Single search entry point
The system SHALL expose one search entry point (`kb_search::search`) used by the
daemon. There is no separate MCP search path in this workspace.

#### Scenario: Backend search goes through the single path
- **WHEN** a query with filters is executed via `Command::Search`
- **THEN** it is served by `kb_search::search` using the over-fetch-then-filter approach
