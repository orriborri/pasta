## Context

`typed-evidence-graph` produces `graph.db`: a derived SQLite index of entities (canonical `EntityRef`) and relations (`Relation` with `evidence_record_id` + `Derivation`), rebuildable from Parquet. `GraphStore` already exposes `relations_by_subject`, `relations_by_object`, `relations_by_evidence`, `has_entity`, `entity_count`, and `relation_count`. Parquet (via `ParquetStore::read_all`) remains the source of truth for full `Record`s.

The only external query surface is the MCP `search_knowledge` tool, which concatenates hybrid-search hits into one string. There is no way to resolve an entity, walk its relations, fetch the record behind a relation, or order events in time. rmcp 1.7 supports structured tool output: a tool returning `rmcp::handler::server::wrapper::Json<T>` (where `T: Serialize + JsonSchema`) emits `structured_content`. That is the mechanism this change uses to return typed responses.

`search-relevance-feedback` (separate, in-flight) already plans to add `record_id` to search results and a feedback path. This change must integrate with that rather than duplicate it: structured search surfaces the same `record_id` as a typed field, and does not re-invent feedback storage.

Constraints inherited from the graph change and the project:
- Read-only: the query layer never fetches, never mutates Parquet or the graph.
- Parquet is the source of truth for record bodies; the graph answers relationship questions.
- Every relation returned carries its `evidence_record_id` and `Derivation`, so a consumer can always trace a claim to a record.

## Goals / Non-Goals

**Goals:**
- Typed, structured query results for entities, relations, evidence, context, and timelines.
- MCP tools that return `structured_content`, not prose.
- Structured search hits (typed fields incl. `record_id`).
- CLI parity for manual inspection.
- Reuse of `typed-evidence-graph` types and `search-relevance-feedback`'s `record_id` work.

**Non-Goals:**
- Natural-language / semantic query planning (explicitly deferred).
- New relation kinds, derivation rules, or graph writes.
- Graph mutation, entity merging, or identity unification.
- Neo4j/RDF/OWL/LangGraph/LLM machinery.
- Pagination beyond simple limits for v1.

## Decisions

### 1. A dedicated read-only `kb-query` crate
The query layer lives in a new `crates/kb-query` depending on `kb-core`, `kb-storage`, and `kb-pipeline` (for search). It owns the request/response types and the functions `get_entity`, `get_related`, `get_evidence`, `get_context`, `get_timeline`, and `search`. **Rationale:** keeps MCP (`kb-mcp`) and CLI (`kb-cli`) thin — both call the same functions — and isolates the typed API from transport. **Alternative:** put it in `kb-storage` — rejected; `kb-storage` is the index layer, and mixing a public query API with storage internals blurs the boundary. **Alternative:** put it only in `kb-mcp` — rejected; the CLI needs the same logic, and duplicating it invites drift.

### 2. Typed response structs, serialized once
```rust
pub struct EntityView { pub entity: EntityRef, pub present: bool, pub out_degree: usize, pub in_degree: usize }

pub struct RelationView {
    pub subject: EntityRef, pub predicate: RelationKind, pub object: EntityRef,
    pub evidence_record_id: String, pub derivation: Derivation, pub created_at: DateTime<Utc>,
}

pub struct EvidenceView { pub record_id: String, pub source: String, pub kind: String,
    pub title: String, pub snippet: String, pub url: String, pub created_at: String }

pub struct RelatedView { pub entity: EntityView, pub relations: Vec<RelationView> }
pub struct ContextView { pub entity: EntityView, pub relations: Vec<RelationView>, pub evidence: Vec<EvidenceView> }
pub struct TimelineEntry { pub at: DateTime<Utc>, pub record: EvidenceView, pub relation: Option<RelationView> }
pub struct SearchHit { pub record_id: String, pub source: String, pub title: String, pub snippet: String, pub url: String, pub created_at: String, pub score: f32 }
```
All derive `Serialize` + `schemars::JsonSchema` so they flow straight through rmcp's `Json<T>`. **Rationale:** one representation used by MCP (structured JSON), CLI (pretty-print), and any future HTTP surface. **Alternative:** separate DTOs per transport — rejected as needless duplication.

### 3. Evidence resolution via `ParquetStore::get_by_ids`
`get_evidence` and `get_context` need full record bodies for a set of `evidence_record_id`s. Add `ParquetStore::get_by_ids(&[&str]) -> Vec<Record>` that scans Parquet and filters by id set. **Rationale:** Parquet is the source of truth for record content; a set-filter scan reuses the existing `read_all` machinery and needs no new index. **Trade-off:** a full scan is O(records) per call; acceptable for v1 volumes (thousands of records) and mirrors how `kb recent` already scans. **Alternative:** maintain an id→record index — deferred; premature until profiling shows the scan is a bottleneck.

### 4. `get_related` returns incident relations with direction
`get_related(entity, kind_filter, limit)` unions `relations_by_subject` and `relations_by_object`, tags each with direction implicitly (subject vs object is visible in the `RelationView`), optionally filters by `RelationKind`, sorts deterministically by `(created_at, relation key)`, and truncates to `limit`. **Rationale:** "what is connected to X" needs both directions; determinism keeps output stable for tests and diffing. A new `GraphStore::relations_incident(&EntityRef)` helper wraps the union so the crate boundary stays clean.

### 5. `get_context` composes, it does not add new data
`get_context(entity, limit)` = `get_entity` + top-`limit` `get_related` + `get_evidence` for the distinct evidence ids of those relations, assembled into one `ContextView`. **Rationale:** agents want a single call that returns "the entity, what it's linked to, and the proof" without N round-trips. It introduces no new facts — every element is already in the graph/Parquet.

### 6. `get_timeline` orders evidence records by time
`get_timeline(entity, limit)` collects the relations incident to the entity, resolves their evidence records, and emits `TimelineEntry` sorted by the record's `created_at` (ties broken by record id). Each entry carries the record and, where the entry originates from a specific relation, that relation. **Rationale:** "what happened with X, in order" is the natural evidence-graph view; ordering by the evidence record's timestamp keeps it grounded in real events. **Alternative:** order by relation `created_at` — same value in practice (relation timestamps are copied from records), but ordering by the record is clearer for consumers.

### 7. Structured search reuses hybrid search; integrates feedback's `record_id`
`search(query, source_filter, limit)` calls the existing `hybrid_search`, maps each `SearchResult` to a typed `SearchHit` (carrying `record_id`), and applies the optional source filter. **Rationale:** no new ranking logic; it exposes the fields (notably `record_id`) that `search-relevance-feedback` also relies on. The two changes share the field, not the implementation, avoiding duplicate `record_id` plumbing. **Alternative:** re-implement search — rejected.

### 8. MCP tools return `Json<T>`; errors as typed error results
Each tool returns `Result<Json<T>, rmcp::ErrorData>`. The existing `search_knowledge` tool is converted to return `Json<SearchResponse>` (a list of `SearchHit`) instead of a `String`. New tools: `get_entity`, `get_related`, `get_evidence`, `get_context`, `get_timeline`. **Rationale:** `Json<T>` places results in `structured_content` with a schema, which is exactly "typed/structured MCP responses rather than prose-only blobs." **Trade-off:** clients that only read the text `content` field see less; acceptable and standard for structured tools. Params structs derive `Deserialize + JsonSchema` as the existing tool already does.

### 9. CLI parity
Add `kb entity <ref>`, `kb related <ref> [--kind K] [--limit N]`, `kb evidence <record-id>`, `kb context <ref> [--limit N]`, `kb timeline <ref> [--limit N]`, each printing the structured result (human-readable, with `--json` to dump the typed payload). **Rationale:** the same query layer, inspectable from the terminal; also gives the tests a non-MCP entry point.

## Risks / Trade-offs

- [Risk] `get_by_ids` full scan cost grows with corpus → **Mitigation:** batch all needed ids into one scan per query; revisit with an index if profiling demands. Documented as a v1 trade-off.
- [Risk] Entity referenced by relations but absent as a record → **Mitigation:** `EntityView.present=false`; `get_evidence` returns only the records that exist; the relation (and its evidence pointer) is still returned so the trail is intact.
- [Risk] Divergence with `search-relevance-feedback` on the `SearchHit` shape → **Mitigation:** keep `record_id` the canonical field name both changes use; coordinate the struct in `kb-query` so feedback consumes it rather than defining its own.
- [Risk] rmcp structured output not surfaced by some clients → **Mitigation:** `Json<T>` still yields a valid `CallToolResult`; clients that ignore `structured_content` degrade gracefully. Behavior verified against the rmcp 1.7 `Json` wrapper.
- [Trade-off] No pagination beyond `limit` in v1 → accepted; `limit` + deterministic ordering is enough for current use.

## Migration Plan

1. Add `ParquetStore::get_by_ids` + `GraphStore::relations_incident`/`entity_degree` with unit tests.
2. Create `kb-query` with the response types and the five query functions + structured `search`; unit/integration tests against a temp graph + Parquet fixture.
3. Convert `kb-mcp` `search_knowledge` to structured `Json<SearchResponse>`; add the five typed tools.
4. Add the CLI subcommands.
5. Full workspace fmt(new files)/check/clippy/test + strict OpenSpec validation.

Rollback: the query layer and new tools are additive; reverting the `kb-mcp`/`kb-cli` wiring and dropping `kb-query` restores the prior prose `search_knowledge` with no data impact (nothing is written).

## Open Questions

- Should `get_related` expose an explicit `direction: In|Out` field, or leave it inferable from subject/object vs the queried entity? (Leaning: add the explicit field; it is cheap and removes ambiguity for consumers.)
- Should `search_knowledge` keep a human-readable text `content` alongside `structured_content` for backward compatibility, or structured-only? (Leaning: include a short text summary for graceful degradation.)
- Does `search-relevance-feedback` want to consume `kb_query::SearchHit` directly, or keep its own row type? (Coordinate before both land; not blocking this change's internal shape.)
