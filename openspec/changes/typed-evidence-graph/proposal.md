## Why

Pasta's knowledge base is a flat pool of records with weak, string-typed links: `extract` writes entity mentions like `linear:AB-123` into `Record.entities`, and `cross_dedupe` merges records that happen to share one. There is no way to ask "which commits reference this ticket", "who authored the messages in this thread", or "what evidence connects this MR to that issue". Relationships are implicit, untyped, and lost the moment two records fail to dedupe.

At the same time, the planned `fetcher-modes-and-resolver` change introduces an `EntityRef` type and a resolver that pulls referenced entities on demand — producing exactly the raw material a graph needs, but with nowhere structured to put the edges it discovers.

A typed evidence graph turns those implicit mentions into explicit, typed, provenance-bearing relationships **without** inventing facts: every edge is either stated directly in a record or deterministically derived from record fields, and every edge names the record that is its evidence. This is the foundation the structured evidence API (a separate change) queries.

## What Changes

- Promote a canonical **`EntityRef`** and **`EntityKind`** into `kb-core` as the single entity representation shared by extraction, the resolver, and the graph. The planned `fetcher-modes-and-resolver` change reuses this type rather than defining its own.
- Add **`RelationKind`**, **`Relation`**, and **`Derivation`** types in `kb-core`. Every `Relation` carries an `evidence_record_id` and a `Derivation` explaining how it was produced (explicit statement vs. named deterministic rule).
- Add a **deterministic relation extractor** (`kb-pipeline`) that turns records into relations using only explicit fields and pure, reproducible rules (mention edges, authorship, participation, thread membership, source-native links). No LLM, no similarity, no heuristics that vary run to run.
- Add a **SQLite graph store** (`graph.db`) in `kb-storage` as a **derived index** — rebuildable at any time from Parquet, exactly like Tantivy and LanceDB. It stores entities, relations, and their evidence pointers.
- Rebuild the graph as part of **`kb reindex`** (and reprocess), reading Parquet as the source of truth and regenerating `graph.db` deterministically.
- Add **comprehensive determinism and rebuild tests**: extracting relations from the same records twice yields byte-identical relation sets; a rebuild from Parquet reproduces the graph; every relation has a resolvable `evidence_record_id`.

This change does **not** add any query API, MCP surface, or CLI query command — those live in the `structured-evidence-api` change. It also does not add Neo4j/RDF/OWL, LangGraph, LLM relation extraction, or any semantic-similarity-derived edge.

## Capabilities

### New Capabilities
- `evidence-graph`: A typed, provenance-bearing graph of entities and relations, derived deterministically from records, in which every relation names the record that evidences it.
- `graph-index`: A SQLite `graph.db` treated as a rebuildable derived index alongside Tantivy and LanceDB, regenerated from Parquet during reindex.

### Modified Capabilities
- `arrow-storage`: Parquet remains the sole source of truth; reindex now also rebuilds the graph derived index from it, and the reindex contract is extended to cover graph regeneration.

## Impact

- `crates/kb-core/src/` — new `entity.rs` (`EntityRef`, `EntityKind`) and `relation.rs` (`RelationKind`, `Relation`, `Derivation`); `lib.rs` re-exports. No change to `Record` fields.
- `crates/kb-pipeline/src/` — new `relation_extract.rs` (pure `relations_from_records(&[Record]) -> Vec<Relation>`); shared `entity_refs` parsing lifted to use the canonical `EntityRef`.
- `crates/kb-storage/src/` — new `graph_store.rs` (`GraphStore` over `graph.db`): schema, `rebuild(records, relations)`, and read helpers; `lib.rs` re-export; `KbConfig::graph_path()` in `kb-core/config.rs`.
- `crates/kb-cli/src/main.rs` — `cmd_reindex`/`cmd_reprocess` also rebuild `graph.db` from `parquet.read_all()`.
- `crates/kb-sync/src/lib.rs` — incremental `index` upserts newly derived relations into the graph after storage.
- Depends on the `EntityRef` concept from `fetcher-modes-and-resolver`; this change owns the canonical definition and that change is expected to consume it.
