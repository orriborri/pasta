## ADDED Requirements

### Requirement: MCP server exposes knowledge base tools
The system SHALL provide an MCP stdio server with tools: search, get_context, list_sources, get_entity.

#### Scenario: AI tool searches knowledge base
- **WHEN** an MCP client calls `search_knowledge` with query "EKS upgrade process"
- **THEN** it returns hybrid search results with source links, snippets, and relevance scores

### Requirement: CLI interface for sync and search
The system SHALL provide CLI commands: `kb sync`, `kb search`, `kb stats`, `kb reindex`.

#### Scenario: Manual sync
- **WHEN** user runs `kb sync --source slack --since 2026-06-01`
- **THEN** Slack messages from June 1 onward are fetched, processed, and indexed

#### Scenario: CLI search
- **WHEN** user runs `kb search "deployment rollback procedure" --limit 5`
- **THEN** top 5 hybrid search results are printed with source, date, and snippet

### Requirement: Pasta integration via CLI or library
Pasta SHALL call kb-engine as either a CLI subprocess (`kb sync --incremental`) or link it as a Rust library crate for in-process search.

#### Scenario: Pasta schedules sync
- **WHEN** pasta's scheduler triggers kb-sync schedule
- **THEN** it executes `kb sync --incremental` which fetches only new data since last cursor
