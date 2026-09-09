## MODIFIED Requirements

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
