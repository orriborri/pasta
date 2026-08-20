# Design Document

## Context

pasta's `kb-engine` has a sound raw-data and retrieval foundation: source
fetchers normalize records, Parquet is the source of truth, LanceDB provides
semantic retrieval, Tantivy provides lexical retrieval, and SQLite holds sync
state. Hybrid results are currently merged with Reciprocal Rank Fusion.

The missing abstraction is persistent structure between records. A Linear issue,
GitHub pull request, Slack discussion, project note, and code symbol may all be
about the same unit of work, but today the caller receives search results and
must reconstruct those relationships repeatedly.

This design adds an entity/relation layer without replacing the existing corpus
or indexes. The graph is a derived projection over authoritative records and
external deterministic code structure. It is accessed through a unified
retrieval service, so pasta's orchestration layer and MCP clients do not depend
on graph storage, Tantivy, LanceDB, or Graphify directly.

## Goals / Non-Goals

**Goals:**
- Introduce stable entities, typed relations, and evidence-bearing provenance
- Keep Parquet as the authoritative ingested corpus
- Put lexical, semantic, and graph retrieval behind one service boundary
- Allow graph expansion to enrich fuzzy search without making it mandatory
- Expose agent-oriented context/relationship operations through MCP
- Evaluate Graphify as an optional code-graph provider behind an adapter
- Preserve rebuildability and incremental ingestion

**Non-Goals:**
- Replacing Parquet, Tantivy, LanceDB, or SQLite in this change
- Migrating all existing records into entities before graph-aware retrieval ships
- Making an LLM-generated knowledge graph authoritative
- Reimplementing Graphify inside pasta
- Removing Tree-sitter before a measured Graphify comparison exists
- Moving task/PARA/workflow ownership from pasta into the knowledge engine
- Choosing a graph database before corpus/traversal measurements exist

## Architectural Boundary

```text
 Slack   Gmail   Linear   Calendar   Docs   Vault
   │       │       │         │        │      │
   └───────┴───────┴─────────┴────────┴──────┘
                       │
                 source adapters
                       │
                       ▼
                KnowledgeRecord
                       │
          ┌────────────┼─────────────┐
          │            │             │
          ▼            ▼             ▼
       Parquet      extractors    text/vector
       (truth)          │          indexes
                        ▼
                 Entity + Relation
                        │
                        ▼
                 Graph Repository
                        ▲
                        │
 Repository ──▶ CodeGraphProvider
                    (Graphify candidate)

       Tantivy ─┐
       LanceDB ─┼──▶ KnowledgeService ──▶ kb-cli
       Graph ───┘                       ├──▶ kb-mcp
                                       └──▶ pasta backend
```

The important direction of dependency is inward: Graphify, Tantivy, LanceDB,
and a future graph store implement infrastructure contracts. `kb-core` domain
types do not import their types.

## Domain Model

The initial model lives in `kb-core` because identity, relation semantics, and
provenance are knowledge-domain concepts rather than storage concepts.

```rust
pub struct EntityId(String);

pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub title: String,
    pub attributes: BTreeMap<String, Value>,
    pub sources: Vec<SourceRef>,
}

pub enum EntityKind {
    Person,
    Project,
    Task,
    Issue,
    PullRequest,
    Repository,
    Document,
    Message,
    Meeting,
    CodeSymbol,
}

pub struct Relation {
    pub from: EntityId,
    pub kind: RelationKind,
    pub to: EntityId,
    pub provenance: Provenance,
}

pub enum Provenance {
    Explicit { evidence: SourceRef },
    Extracted { evidence: SourceRef, extractor: String },
    Inferred { evidence: Vec<SourceRef>, model: String, confidence: f32 },
}
```

The exact Rust representation may change during implementation, but these
semantics are part of the public contract.

### Identity

Entity IDs must be deterministic when a source provides a stable identity. Use
a namespaced canonical key rather than a random UUID, for example:

```text
linear:issue:ENG-123
github:repo:orriborri/pasta
github:pr:orriborri/pasta#42
vault:project:pasta
code:orriborri/pasta:crates/kb-core/src/entity.rs::Entity
```

Cross-source identity resolution is deliberately incremental. `linear:issue:X`
and a vault task referring to X may initially remain separate entities connected
by a relation. A later resolver may merge/alias them only when deterministic
evidence exists.

## Provenance Rules

The graph must distinguish facts from extraction and inference.

- **Explicit**: the source itself declares the edge. Example: a GitHub PR body
  links a Linear issue, or a task frontmatter names a project.
- **Extracted**: deterministic parser/tool derives the edge. Example: Graphify or
  Tree-sitter reports that function A calls B.
- **Inferred**: a model proposes a relationship from evidence. These edges carry
  model identity and confidence and must never silently replace explicit facts.

Graph queries return provenance by default internally. User-facing callers may
choose a compact projection, but MCP must be able to request evidence.

## Extraction Pipeline

Do not make graph construction a prerequisite for ingesting a record.

```text
fetch
  ↓
normalize
  ↓
filter/dedupe
  ↓
write raw record ───────────────▶ Parquet
  │
  ├──▶ lexical/vector indexing
  │
  └──▶ entity/relation extractors
             │
             └── failure is isolated/logged
```

This preserves the existing property that a source remains useful even if its
entity extractor is incomplete.

Initial deterministic extractors should target high-value, low-ambiguity edges:

- Linear issue → project/team when present in source metadata
- GitHub PR → repository
- GitHub PR → issue when an explicit issue key/link is present
- Vault task → project/initiative from path/frontmatter
- Message/document → person from explicit author identity

Do not begin with broad LLM inference. Establish deterministic graph value first.

## Graph Repository

Define a repository contract in the knowledge layer, implemented in
`kb-storage`:

```rust
trait GraphRepository {
    async fn upsert_entities(&self, entities: &[Entity]) -> Result<()>;
    async fn upsert_relations(&self, relations: &[Relation]) -> Result<()>;
    async fn entity(&self, id: &EntityId) -> Result<Option<Entity>>;
    async fn related(&self, query: RelatedQuery) -> Result<Vec<RelatedEntity>>;
    async fn clear_derived(&self, schema_version: u32) -> Result<()>;
}
```

Do not select a specialized graph database initially. Implement the contract
with the smallest storage mechanism that supports the measured corpus and test
workload. SQLite is acceptable for the prototype if graph tables are clearly
separated from sync state and documented as derived data. A dedicated graph
store is justified only by measurements.

## Unified Retrieval Service

`KnowledgeService` becomes the public retrieval boundary.

Conceptually:

```rust
trait KnowledgeService {
    async fn search(&self, query: SearchQuery) -> Result<Vec<KnowledgeResult>>;
    async fn entity(&self, id: &EntityId) -> Result<Option<Entity>>;
    async fn related(&self, query: RelatedQuery) -> Result<Vec<RelatedEntity>>;
    async fn context(&self, query: ContextQuery) -> Result<EntityContext>;
}
```

`search` remains backward compatible with existing hybrid retrieval. Graph-aware
search is an option/strategy layered on top:

```text
natural-language query
        │
        ├── Tantivy lexical search
        ├── LanceDB semantic search
        │
        ▼
      RRF merge
        │
        ▼
  identify seed entities
        │
        ▼
 bounded graph expansion
        │
        ▼
 rank + annotate reasons
```

Graph expansion has hard limits for depth and fan-out. Initial default depth is
1. Depth >2 should require an explicit caller request until benchmarks show it
is useful and safe.

Every result records a retrieval reason such as lexical match, semantic match,
direct relation, or graph path. This makes retrieval debuggable.

## Code Graph Adapter / Graphify

Graphify is treated as a candidate provider, not a foundation dependency.

```rust
trait CodeGraphProvider {
    async fn index_repository(&self, repo: &RepositoryRef) -> Result<IndexReport>;
    async fn entities(&self, repo: &RepositoryRef) -> Result<Vec<Entity>>;
    async fn relations(&self, repo: &RepositoryRef) -> Result<Vec<Relation>>;
}
```

The adapter translates Graphify's representation into pasta's `Entity` and
`Relation` types. No Graphify-specific node/edge type crosses into `kb-core`.

During evaluation, run the existing Tree-sitter extraction and Graphify against
the same repositories and compare:

- languages and symbol coverage
- call/import/inheritance edge correctness
- incremental update latency
- initial indexing/rebuild time
- query usefulness for real coding questions
- binary/runtime/deployment complexity
- failure behaviour when the provider is absent or stale

After the benchmark, choose exactly one authoritative code-structure provider in
production. Keeping two independent call graphs indefinitely would create the
same accreted-architecture problem pasta is already removing elsewhere.

## MCP Surface

Extend `kb-mcp` around user intent rather than storage primitives:

- `search_knowledge(query, options)` — fuzzy retrieval, optionally graph-expanded
- `get_entity(id)` — canonical entity metadata and sources
- `find_related(id, relation?, depth?)` — bounded traversal
- `get_context(id_or_query, budget?)` — compact agent context combining records,
  entity metadata, relations, and provenance

Possible later tools such as `trace_relationship` or `recent_activity` should be
added only after concrete agent workflows demonstrate need.

The CLI may expose diagnostic equivalents (`kb entity`, `kb related`,
`kb context`) but calls the same service.

## Rebuild Strategy

Parquet remains authoritative for ingested records. Derived graph data carries a
schema version. A graph rebuild:

1. clears derived entities/relations for the target schema,
2. scans authoritative records,
3. reruns deterministic entity/relation extractors,
4. optionally asks configured external code graph providers to rebuild/import,
5. validates dangling-edge and duplicate-edge invariants.

Inferred edges should either be rebuilt from their evidence/model version or be
stored in a separately identifiable derived layer; they are never silently
promoted to raw truth.

## Phased Delivery

### Phase 1 — Domain model

Add entity/relation/provenance types, deterministic ID helpers, serialization,
and unit/property tests. No search behaviour changes.

### Phase 2 — Graph repository + service boundary

Add graph persistence behind `GraphRepository`. Introduce `KnowledgeService` and
route existing hybrid search through it without changing ranking.

### Phase 3 — Deterministic relationships

Extract high-confidence Linear/GitHub/Vault/person relationships and make graph
rebuild incremental/reproducible.

### Phase 4 — Agent retrieval

Add bounded graph expansion and MCP entity/related/context tools. Benchmark
hybrid-only vs graph-aware retrieval on real questions.

### Phase 5 — Graphify evaluation

Implement the Graphify adapter as an optional provider, benchmark against the
existing Tree-sitter path, and record an adopt/reject decision. Remove duplicate
code-graph machinery only in a subsequent change after that decision.

## Risks

- **Graph becomes another source of truth.** Mitigation: graph is explicitly
  derived, schema-versioned, and rebuildable from Parquet/provider inputs.
- **Entity resolution creates false merges.** Mitigation: deterministic IDs and
  relations first; cross-source merging requires explicit evidence.
- **Graph expansion harms search relevance.** Mitigation: bounded depth/fan-out,
  retrieval reasons, fallback to current hybrid search, and evaluation fixtures.
- **Graphify becomes an infrastructure dependency too early.** Mitigation:
  optional adapter and benchmark gate; no Graphify types in the domain model.
- **Two code parsers drift.** Mitigation: comparison is temporary and ends with
  an explicit single-provider decision.
- **Too many storage engines.** Mitigation: do not add a graph database before
  measurement; start behind a repository contract and record operational data.
