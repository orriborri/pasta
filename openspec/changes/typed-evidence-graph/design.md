## Context

Records today are flat rows persisted to Parquet (source of truth) and projected into two derived indexes: Tantivy (`text_index`) for full-text and LanceDB (`vector_store`) for vectors. Both are cleared and rebuilt from `ParquetStore::read_all()` by `kb reindex`. Relationships between records are only implicit: `ExtractStage` writes mention strings (`linear:AB-123`, `mr:!2935`) into `Record.entities`, and `CrossSourceDedupeStage` collapses records sharing a mention into one. Once two records fail to merge, the connection is gone; and there is never a typed edge such as "commit X references issue AB-123" or "person P authored message M".

The `fetcher-modes-and-resolver` change (not yet implemented) plans an `EntityRef { kind, id, project }` parsed from those same mention strings, plus a resolver loop that pulls missing entities. That is the natural producer of graph nodes and edges, but there is no typed, provenance-bearing place to store them. This change builds that place and the deterministic extractor that fills it, and it owns the canonical `EntityRef`/`EntityKind` so the resolver reuses one representation.

Hard constraints for this change:
1. **Parquet stays the only source of truth.** The graph is a derived index; deleting `graph.db` and rebuilding from Parquet must fully reproduce it.
2. **Every relation carries `evidence_record_id`.** An edge with no record backing it cannot exist.
3. **Relations are explicit or deterministically derived.** Extraction is a pure function of record fields; the same inputs always yield the same relations.
4. **Semantic similarity never creates a factual relation.** Vectors are for retrieval ranking only; they are not consulted by the extractor.

## Goals / Non-Goals

**Goals:**
- One canonical `EntityRef`/`EntityKind` in `kb-core`, reused everywhere entities are named.
- Typed `Relation` with `RelationKind`, `evidence_record_id`, and a `Derivation` that records provenance.
- A pure, reproducible relation extractor over records.
- A `graph.db` SQLite derived index that rebuilds from Parquet during reindex.
- Determinism and rebuild proven by tests.

**Non-Goals:**
- Any query API, MCP tool, or CLI query command (owned by `structured-evidence-api`).
- Neo4j, RDF, OWL, property-graph engines, or LangGraph.
- LLM-based or similarity-based relation extraction.
- People identity unification beyond what `EntityRegistry` already resolves (the extractor uses author strings as-is; canonicalization is a resolver concern).
- Changing `Record`'s Parquet schema.

## Decisions

### 1. Canonical `EntityRef` / `EntityKind` in `kb-core`
```rust
pub enum EntityKind { Person, LinearIssue, MergeRequest, Commit, Document, Meeting, SlackThread, Project }

pub struct EntityRef { pub kind: EntityKind, pub id: String, pub project: Option<String> }
```
`EntityRef` has a canonical string form (`kind:id` with optional `@project`) and parses back from it, so it round-trips through `Record.entities` and the graph's `entities` table. **Rationale:** the instructions require reusing the `EntityRef` planned by `fetcher-modes-and-resolver` and promoting it to `kb-core` rather than duplicating. Placing it in `kb-core` lets `kb-pipeline` (extractor), the future fetcher resolver, and `kb-storage` (graph) all depend on one definition. **Alternative considered:** define it in `kb-fetchers` per the original plan — rejected because `kb-storage` and `kb-pipeline` would then depend on `kb-fetchers`, inverting the dependency direction.

### 2. `Relation` with mandatory evidence and explicit `Derivation`
```rust
pub enum RelationKind { References, AuthoredBy, ParticipatedIn, PartOfThread, Mentions, LinkedTo }

pub enum Derivation {
    Explicit,                       // stated by the source record itself (author, participants, thread)
    Rule { name: &'static str },    // deterministic extraction rule (e.g. "mention:linear")
}

pub struct Relation {
    pub subject: EntityRef,
    pub predicate: RelationKind,
    pub object: EntityRef,
    pub evidence_record_id: String, // MUST be non-empty and resolvable
    pub derivation: Derivation,
    pub created_at: DateTime<Utc>,  // taken from the evidence record, for deterministic ordering
}
```
A stable `Relation::key()` (hash of subject, predicate, object, evidence_record_id, derivation) is the graph's dedup/primary key so re-derivation is idempotent. **Rationale:** the evidence pointer and derivation are the invariant that keeps the graph honest — no edge without a backing record, and provenance is queryable. **Alternative:** a free-form `source: String` — rejected as unenforceable; a typed `Derivation` makes "explicit vs derived" a first-class, testable distinction.

### 3. Deterministic extractor as a pure function, not a `PipelineStage`
`relations_from_records(&[Record]) -> Vec<Relation>` lives in `kb-pipeline::relation_extract` and is a pure function: no I/O, no clock reads (timestamps come from records), no RNG, output sorted by `Relation::key()`. Rules for v1:
- **AuthoredBy** (`Explicit`): record → `Person(author)` when author is non-empty/non-placeholder.
- **ParticipatedIn** (`Explicit`): each participant → the record's entity.
- **PartOfThread** (`Explicit`): record → `SlackThread(thread_id)` when `thread_id` is set.
- **Mentions / References** (`Rule`): each parsed `EntityRef` in `Record.entities` → a `Mentions` edge (or `References` for issue/MR mentions from commits/MRs), rule-named per kind (`mention:linear`, `mention:mr`).
- **LinkedTo** (`Rule`): source-native links already present as structured fields (e.g. an MR record whose branch encodes a Linear id) surfaced by extraction — only when the link is present in the record, never inferred.

**Rationale:** purity is what makes determinism and rebuild testable, and keeping it out of the sync `PipelineStage` trait avoids coupling graph derivation to batch pipeline order. **Alternative:** make it a `PipelineStage` — rejected; stages mutate `Record`s in place and run mid-pipeline, whereas relations are derived from the *final* record set and stored separately.

### 4. `graph.db` SQLite as a derived index
Schema:
```sql
CREATE TABLE entities (
    ref TEXT PRIMARY KEY,        -- canonical EntityRef string
    kind TEXT NOT NULL,
    id TEXT NOT NULL,
    project TEXT
);
CREATE TABLE relations (
    key TEXT PRIMARY KEY,        -- Relation::key()
    subject_ref TEXT NOT NULL,
    predicate TEXT NOT NULL,
    object_ref TEXT NOT NULL,
    evidence_record_id TEXT NOT NULL,
    derivation TEXT NOT NULL,    -- "explicit" | "rule:<name>"
    created_at TEXT NOT NULL,
    FOREIGN KEY (subject_ref) REFERENCES entities(ref),
    FOREIGN KEY (object_ref) REFERENCES entities(ref)
);
CREATE INDEX idx_rel_subject ON relations(subject_ref);
CREATE INDEX idx_rel_object  ON relations(object_ref);
CREATE INDEX idx_rel_evidence ON relations(evidence_record_id);
```
`GraphStore::rebuild(records, relations)` is transactional: it drops/recreates rows, inserts entities (from both records and relation endpoints), then relations, in one transaction. `GraphStore::upsert(relations)` handles the incremental sync path with `INSERT OR REPLACE` keyed on `key`. **Rationale:** SQLite matches the existing `state.db` dependency (rusqlite, bundled), needs no new services, and is trivially rebuildable — satisfying the derived-index invariant. It lives at `KbConfig::graph_path()` (`data_dir/graph.db`), sibling to the other indexes. **Alternative:** store the graph inside `state.db` — rejected; `state.db` holds mutable cursors/hashes/feedback that must survive a reindex, whereas the graph must be safe to delete and rebuild.

### 5. Reindex rebuilds the graph from Parquet
`cmd_reindex` and `cmd_reprocess` call `parquet.read_all()`, derive relations via `relations_from_records`, and call `GraphStore::rebuild`. Deleting `graph.db` before rebuild is safe by construction. **Rationale:** enforces invariant #1 — the graph is reproducible from the source of truth and never diverges. The incremental `kb-sync::index` path additionally `upsert`s relations for the changed batch so the graph stays current between reindexes.

### 6. Determinism enforced structurally
The extractor sorts output by `Relation::key()` and derives timestamps from records, so no run-to-run variance is possible. Tests assert: (a) two extractions of the same input are equal; (b) a rebuild from a fixed record fixture yields a known relation set; (c) every emitted relation's `evidence_record_id` matches a record id in the input; (d) no relation is produced without a `Derivation`.

## Risks / Trade-offs

- [Risk] Entity endpoints referenced by relations but absent as records (e.g. a mentioned ticket not yet fetched) → **Mitigation:** insert the endpoint entity from the `EntityRef` itself; the entity exists, the backing record may arrive later via the resolver. Evidence still points at the mentioning record, which does exist.
- [Risk] Author/participant strings are not yet canonical people → **Mitigation:** accept the raw identifier as the `Person` id for v1; identity unification is out of scope and handled upstream by `EntityRegistry`/resolver. The edge is still evidence-backed and deterministic.
- [Risk] Graph and Parquet drift if incremental upsert misses a case → **Mitigation:** reindex is the authority and fully rebuilds; upsert is best-effort convenience. Rebuild tests guard the authoritative path.
- [Trade-off] Storing endpoint entities that lack records → accepted; querying can distinguish "entity known" from "record present" via the `structured-evidence-api`.
- [Risk] `Relation::key()` collisions → **Mitigation:** key is a SHA-256 over all identifying fields, matching the existing `content_hash` approach.

## Migration Plan

1. Land `EntityRef`/`EntityKind` + `Relation`/`RelationKind`/`Derivation` in `kb-core` with unit tests (round-trip, key stability). No downstream consumer yet — additive.
2. Add `KbConfig::graph_path()` and `GraphStore` (schema + `rebuild`/`upsert` + reads) in `kb-storage`, unit-tested against a temp db.
3. Add `relations_from_records` in `kb-pipeline` with determinism tests.
4. Wire `cmd_reindex`/`cmd_reprocess` to rebuild `graph.db`; wire `kb-sync::index` to upsert.
5. Add rebuild/round-trip integration test.

Rollback: the graph is purely additive and derived; deleting `graph.db` and reverting the reindex wiring removes it with zero impact on Parquet, Tantivy, or LanceDB.

## Open Questions

- Should `References` vs `Mentions` be split by the subject's kind (commit/MR → `References`, chat/email → `Mentions`), or unified as `Mentions` for v1 with kind inferred at query time? (Leaning: split deterministically by subject kind, since it is a pure function of the record.)
- Should endpoint entities without a backing record be flagged (`present: bool`) in the `entities` table now, or left for `structured-evidence-api`? (Leaning: leave the column out until the API needs it, to keep this change minimal.)
