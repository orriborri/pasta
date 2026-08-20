# Requirements Document

## Introduction

pasta's knowledge engine currently treats knowledge primarily as records that are
retrieved through full-text and vector indexes. This is effective for fuzzy
questions such as "where did we discuss retries?", but relationships such as
"this pull request implements this issue", "this message discusses this project",
or "this function calls that function" must be reconstructed on every query.

This change introduces a first-class entity and relationship model and a single
retrieval boundary over lexical, semantic, and graph retrieval. It deliberately
does not replace Parquet, Tantivy, or LanceDB in this phase. It also does not
commit pasta to Graphify: code-graph extraction is defined behind an adapter so
Graphify can be evaluated without making it part of pasta's core domain model.

The architectural boundary is:

- Parquet records what was ingested and remains the rebuildable source of truth.
- Tantivy and LanceDB discover relevant records/entities.
- The graph stores explicit relationships and provenance.
- pasta consumes a knowledge API and owns workflow/orchestration rather than
  storage-specific retrieval logic.

## Glossary

- **Knowledge record**: normalized source item produced by the ingestion pipeline
- **Entity**: stable identifiable object such as a Person, Project, Task, Issue,
  PullRequest, Repository, Document, Message, Meeting, or CodeSymbol
- **Relation**: typed directed edge between two entities
- **Provenance**: evidence describing where an entity or relation came from and
  whether it was explicit, extracted, or inferred
- **Graph expansion**: traversal from retrieved seed entities to related entities
- **Retrieval API**: storage-independent interface used by pasta, kb-cli, and
  kb-mcp to search and inspect knowledge
- **Code graph adapter**: optional provider of code entities and structural edges
  such as calls/imports/implements; Graphify is the first candidate implementation

## Requirements

### Requirement 1

**User Story:** As an agent or user, I want knowledge objects to have stable
identities, so that information from different sources can refer to the same
person, project, issue, pull request, or code symbol.

#### Acceptance Criteria

1. WHEN `kb-core` is compiled THEN it SHALL define stable `EntityId`, `Entity`,
   `EntityKind`, `Relation`, and `RelationKind` domain types
2. WHEN entities are created from source records THEN their IDs SHALL be
   deterministic for the same source identity
3. WHEN the initial entity model is complete THEN it SHALL support at minimum
   Person, Project, Task, Issue, PullRequest, Repository, Document, Message,
   Meeting, and CodeSymbol
4. WHEN a source cannot yet be resolved to a richer entity type THEN the record
   SHALL remain searchable without requiring entity creation
5. WHEN entity types are serialized THEN the representation SHALL be versionable
   without depending on a storage backend's native schema

### Requirement 2

**User Story:** As an agent, I want relationships to be stored explicitly, so
that I can traverse known connections instead of asking an LLM to rediscover
them from retrieved chunks.

#### Acceptance Criteria

1. WHEN the graph model is compiled THEN relations SHALL contain `from`, `kind`,
   `to`, and provenance
2. WHEN the initial relation model is complete THEN it SHALL support at minimum
   `belongs_to`, `mentions`, `discussed_in`, `implements`, `modified_by`,
   `depends_on`, `calls`, `imports`, and `attended_by`
3. WHEN the same deterministic relation is extracted twice THEN storage SHALL
   deduplicate it rather than create parallel identical edges
4. WHEN an entity is deleted or rebuilt THEN dangling relations SHALL NOT be
   returned by graph queries
5. WHEN a graph traversal is requested THEN callers SHALL be able to constrain
   relation kind and traversal depth

### Requirement 3

**User Story:** As a user, I want every relationship to be explainable, so that
agent-generated knowledge does not become an untraceable source of truth.

#### Acceptance Criteria

1. WHEN a relation is stored THEN its provenance SHALL classify it as Explicit,
   Extracted, or Inferred
2. WHEN provenance refers to source material THEN it SHALL retain a source
   reference sufficient to locate the supporting record
3. WHEN an Inferred relation is stored THEN it SHALL include a confidence value
4. WHEN an Explicit or deterministic Extracted relation is stored THEN callers
   SHALL NOT be required to invent an LLM confidence score
5. WHEN `kb-mcp` returns a relation THEN it SHALL be possible for the client to
   request the evidence/provenance supporting it

### Requirement 4

**User Story:** As the maintainer, I want one retrieval API, so that pasta and
agents do not depend directly on Tantivy, LanceDB, graph storage, or ranking
implementation details.

#### Acceptance Criteria

1. WHEN the retrieval boundary is implemented THEN pasta-facing code SHALL use a
   single knowledge service interface for search, entity lookup, and relation
   traversal
2. WHEN a search is executed THEN the implementation MAY combine lexical,
   semantic, and graph results without exposing backend-specific result types
3. WHEN an entity is retrieved THEN the result SHALL include stable identity,
   kind, display metadata, and source references
4. WHEN related entities are requested THEN the API SHALL support an optional
   relation filter and bounded traversal depth
5. WHEN backend implementations change THEN `crates/backend` SHALL require no
   storage-specific changes unless the public knowledge contract changes
6. WHEN the workspace is searched outside `kb-storage` THEN new direct Tantivy
   or LanceDB query code SHALL NOT be introduced

### Requirement 5

**User Story:** As a user, I want fuzzy search and structural relationships to
work together, so that broad natural-language queries can lead to precise
context.

#### Acceptance Criteria

1. WHEN hybrid search finds records associated with entities THEN those entities
   SHALL be eligible as graph-expansion seeds
2. WHEN graph expansion is enabled THEN its maximum depth and maximum added
   results SHALL be bounded
3. WHEN graph-derived results are merged with lexical/vector results THEN each
   result SHALL identify why it was included
4. WHEN graph expansion produces no useful entities THEN existing hybrid search
   behaviour SHALL remain available as a fallback
5. WHEN retrieval quality is evaluated THEN a fixture set SHALL compare hybrid
   search alone with hybrid-plus-graph retrieval for the same queries

### Requirement 6

**User Story:** As the maintainer, I want code structure supplied through an
adapter, so that Graphify can be evaluated without coupling pasta's domain model
to one external implementation.

#### Acceptance Criteria

1. WHEN code-graph integration is introduced THEN `kb-core` SHALL define a
   storage/provider-neutral code graph contract
2. WHEN no code graph provider is configured THEN ingestion, hybrid search, and
   non-code graph queries SHALL continue to work
3. WHEN a provider returns code entities THEN they SHALL map to the same
   `Entity`/`Relation` domain types used by all other sources
4. WHEN Graphify is evaluated THEN pasta SHALL consume its output/API through the
   adapter and SHALL NOT make Graphify-specific types part of `kb-core`
5. WHEN Graphify and pasta both extract the same code relationship THEN exactly
   one provider SHALL be authoritative for that relationship in production
6. WHEN the evaluation completes THEN a written benchmark SHALL compare the
   existing Tree-sitter code extraction with Graphify for coverage, correctness,
   update latency, query usefulness, operational complexity, and failure modes
7. UNTIL that benchmark is reviewed THEN existing Tree-sitter dependencies SHALL
   NOT be removed solely because Graphify exists

### Requirement 7

**User Story:** As the maintainer, I want ingestion sources to produce a common
knowledge envelope, so that downstream processing does not branch on Slack,
Gmail, Linear, Git, Calendar, Docs, or Vault unnecessarily.

#### Acceptance Criteria

1. WHEN records leave source-specific fetchers THEN downstream stages SHALL
   receive the existing canonical record plus source metadata sufficient for
   entity extraction
2. WHEN entity/relation extraction runs THEN source-specific parsing SHALL be
   isolated behind extractors rather than spread through retrieval/storage code
3. WHEN a new source is added THEN it SHALL be possible to make its records
   searchable before implementing any source-specific entity extractor
4. WHEN entity extraction fails for one record THEN the raw record SHALL still be
   persisted and indexed
5. WHEN downstream storage is searched THEN it SHALL NOT contain source-specific
   network/API client logic

### Requirement 8

**User Story:** As a Codex or MCP client, I want domain-oriented knowledge tools,
so that I can request context and relationships rather than manually orchestrate
raw searches.

#### Acceptance Criteria

1. WHEN `kb-mcp` is extended THEN it SHALL expose tools equivalent to
   `search_knowledge`, `get_entity`, `find_related`, and `get_context`
2. WHEN `get_context` is called for a known entity THEN it SHALL return the
   entity, directly relevant records, and bounded related entities with
   provenance
3. WHEN a requested entity is unknown THEN the MCP server SHALL return a clear
   not-found result rather than fabricate an entity
4. WHEN a relation is returned through MCP THEN its provenance SHALL be
   serializable in the response
5. WHEN `kb-cli` exposes equivalent diagnostic commands THEN CLI and MCP SHALL use
   the same retrieval service rather than duplicate retrieval logic

### Requirement 9

**User Story:** As the maintainer, I want the graph to remain rebuildable and
operationally understandable, so that it does not become a second hidden source
of truth.

#### Acceptance Criteria

1. WHEN graph data is derived solely from ingested records/code THEN it SHALL be
   rebuildable from authoritative source data
2. WHEN `kb reindex` is extended for graph rebuilding THEN rebuilding SHALL NOT
   mutate the raw Parquet corpus
3. WHEN graph schema/version changes THEN the engine SHALL detect incompatible
   derived graph data and require/recommend a rebuild rather than silently mix
   schemas
4. WHEN sync state is stored in SQLite THEN graph knowledge SHALL NOT be made
   authoritative merely because SQLite is already present
5. WHEN a graph storage technology is selected THEN the decision SHALL be
   documented with measured corpus size, traversal latency, rebuild time, and
   operational complexity

### Requirement 10

**User Story:** As the maintainer, I want this architecture introduced in small,
measurable slices, so that a graph experiment cannot destabilize the existing
knowledge engine.

#### Acceptance Criteria

1. WHEN Phase 1 is complete THEN entity/relation/provenance domain types and unit
   tests SHALL exist without changing existing search results
2. WHEN Phase 2 is complete THEN a graph repository and retrieval API SHALL exist
   behind tests while hybrid search remains functional
3. WHEN Phase 3 is complete THEN at least Linear/GitHub/Vault relationships SHALL
   be extracted from deterministic source metadata where available
4. WHEN Phase 4 is complete THEN MCP SHALL expose graph-aware context tools
5. WHEN Phase 5 is complete THEN Graphify SHALL have been evaluated as the code
   graph provider and either adopted or rejected with evidence
6. AT THE END of every phase THEN `cargo build --workspace`,
   `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` SHALL
   pass
7. WHEN the change is complete THEN Parquet SHALL remain the authoritative raw
   corpus and existing `kb search` SHALL remain usable
