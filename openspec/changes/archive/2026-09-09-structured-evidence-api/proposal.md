## Why

The `typed-evidence-graph` change gives Pasta a typed, provenance-bearing graph of entities and relations, but nothing exposes it. The only query surface today is the MCP `search_knowledge` tool, which returns a single prose blob (`"[source] date | title\nsnippet"`). An AI agent consuming that cannot follow a relation, fetch the evidence behind a claim, enumerate what is connected to an entity, or build a timeline — it gets flattened text and must re-parse it heuristically.

To make the evidence graph useful, the knowledge base needs a small, typed query API: given an entity, return its relations; given a relation or claim, return the record that evidences it; given an entity, return a time-ordered view of what happened. Responses must be **structured** (typed JSON), not prose, so agents can navigate the graph deterministically and cite evidence precisely.

## What Changes

- Add a **structured query layer** (`kb-query`) over the graph (`graph.db`) and Parquet (source of truth) exposing typed results:
  - **`get_entity`** — resolve a canonical `EntityRef`, returning the entity plus its degree and whether a backing record exists.
  - **`get_related`** — the relations incident to an entity (in/out, optionally filtered by `RelationKind`), each with its evidence pointer and derivation.
  - **`get_evidence`** — resolve an `evidence_record_id` (or a specific relation) to the underlying record(s) that evidence it.
  - **`get_context`** — a bundle for an entity: the entity, its top relations, and the evidence records behind them, in one structured payload.
  - **`get_timeline`** — a time-ordered list of records/relations touching an entity, for "what happened with X" views.
- Make **search results structured**: return typed hits (`record_id`, source, title, snippet, score, url, created_at) instead of a prose blob, so downstream tools can act on fields including `record_id`.
- Expose all of the above as **typed MCP tools** returning `structured_content` (via rmcp's `Json<T>` wrapper) rather than strings. Add matching **CLI subcommands** for parity and manual inspection.
- Integrate with `search-relevance-feedback`: reuse its `record_id`-in-results work rather than introducing a parallel change; structured search simply surfaces the same `record_id` in a typed field.

This change does **not** add the natural-language semantic query planner — that is deferred until the graph and this API prove stable. It adds no new graph relations, no new derivation rules, and no Neo4j/RDF/OWL/LangGraph/LLM machinery.

## Capabilities

### New Capabilities
- `evidence-query`: A typed query layer over the evidence graph and Parquet returning `get_entity`, `get_related`, `get_evidence`, `get_context`, and `get_timeline` as structured results, each relation carrying its evidence and derivation.

### Modified Capabilities
- `kb-api`: The MCP server returns structured (`structured_content`) responses instead of prose; `search_knowledge` returns typed hits including `record_id`; new `get_entity`, `get_related`, `get_evidence`, `get_context`, `get_timeline` tools are added, with matching CLI subcommands.

## Impact

- New `crates/kb-query/` — typed request/response structs and query functions over `GraphStore` + `ParquetStore`; no fetching, read-only.
- `crates/kb-storage/src/parquet_store.rs` — add `get_by_ids(&[&str]) -> Vec<Record>` for evidence resolution; add `GraphStore` read helpers as needed (e.g. `relations_incident`, `entity_degree`).
- `crates/kb-mcp/src/main.rs` — add typed tools returning `Json<T>`; convert `search_knowledge` to a structured response.
- `crates/kb-cli/src/main.rs` — add `kb entity`, `kb related`, `kb evidence`, `kb context`, `kb timeline` subcommands printing the structured results.
- Depends on `typed-evidence-graph` (the graph and its `EntityRef`/`Relation` types) and integrates with `search-relevance-feedback` (shared `record_id` in results).
