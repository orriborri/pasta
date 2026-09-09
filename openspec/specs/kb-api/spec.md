# KB API

## Purpose

Standalone query interfaces for the knowledge base: MCP server, CLI, and HTTP API.
## Requirements
### Requirement: MCP server exposes knowledge base tools
The system SHALL provide an MCP stdio server whose tools return structured (`structured_content`) responses rather than prose blobs. It SHALL expose `search_knowledge` (typed hits including `record_id`), `get_entity`, `get_related`, `get_evidence`, `get_context`, and `get_timeline`.

#### Scenario: AI tool searches knowledge base
- **WHEN** an MCP client calls `search_knowledge` with query "EKS upgrade process"
- **THEN** it returns structured hits, each with `record_id`, source, title, snippet, url, timestamp, and relevance score

#### Scenario: AI tool resolves an entity's relations
- **WHEN** an MCP client calls `get_related` for an entity
- **THEN** it returns structured relations, each carrying its endpoints, `evidence_record_id`, and derivation

#### Scenario: AI tool fetches the evidence behind a relation
- **WHEN** an MCP client calls `get_evidence` with an `evidence_record_id` from a relation
- **THEN** it returns the structured evidence record backing that relation

#### Scenario: AI tool requests entity context
- **WHEN** an MCP client calls `get_context` for an entity
- **THEN** it returns a structured bundle of the entity, its relations, and the evidence records behind them

#### Scenario: AI tool requests an entity timeline
- **WHEN** an MCP client calls `get_timeline` for an entity
- **THEN** it returns a structured, time-ordered list of the evidence records touching that entity

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

