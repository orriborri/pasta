## 1. Storage read helpers

- [x] 1.1 Add `ParquetStore::get_by_ids(&[&str]) -> Result<Vec<Record>>` (scan Parquet, filter by id set)
- [x] 1.2 Add `GraphStore::relations_incident(&EntityRef) -> Result<Vec<StoredRelation>>` (union of subject + object, deduped by relation key)
- [x] 1.3 Add `GraphStore::entity_degree(&EntityRef) -> Result<(usize, usize)>` returning (out_degree, in_degree)
- [x] 1.4 Unit tests for the three helpers against temp stores
- [x] 1.5 `cargo check -p kb-storage` + tests green

## 2. kb-query crate: types + query functions

- [x] 2.1 Create `crates/kb-query` (deps: kb-core, kb-storage; workspace lints); add to workspace members
- [x] 2.2 Define response types (`Serialize` + `schemars::JsonSchema`): `EntityView`, `RelationView`, `EvidenceView`, `RelatedView`, `ContextView`, `TimelineEntry`, `SearchHit`
- [x] 2.3 Implement `get_entity(config, &EntityRef) -> Result<EntityView>` (present via record lookup, degrees via `entity_degree`)
- [x] 2.4 Implement `get_related(config, &EntityRef, kind_filter, limit) -> Result<RelatedView>` (incident relations, filter, deterministic sort, limit)
- [x] 2.5 Implement `get_evidence(config, &[record_id]) -> Result<Vec<EvidenceView>>` via `get_by_ids`
- [x] 2.6 Implement `get_context(config, &EntityRef, limit) -> Result<ContextView>` (compose entity + related + evidence)
- [x] 2.7 Implement `get_timeline(config, &EntityRef, limit) -> Result<Vec<TimelineEntry>>` (evidence records ordered by `created_at`, stable tie-break)
- [x] 2.8 Implement `search(config, query, source_filter, limit) -> Result<Vec<SearchHit>>` mapping `hybrid_search` results (carry `record_id`)
- [x] 2.9 Re-export public API from `kb-query/src/lib.rs`

## 3. kb-query tests

- [x] 3.1 Integration test fixture: records → parquet write + graph rebuild in a temp `KbConfig`
- [x] 3.2 Test `get_entity`: present entity degrees; endpoint-without-record `present=false`
- [x] 3.3 Test `get_related`: both directions; kind filter; determinism across two calls
- [x] 3.4 Test `get_evidence`: present id returns view; absent id returns empty (no fabrication)
- [x] 3.5 Test `get_context`: every relation has evidence id; every evidence view maps to a stored record
- [x] 3.6 Test `get_timeline`: ordered by `created_at`; deterministic across two calls

## 4. MCP structured tools

- [x] 4.1 Add typed param structs (`Deserialize` + `JsonSchema`) for the new tools
- [x] 4.2 Convert `search_knowledge` to return `Json<SearchResponse>` (typed hits incl. `record_id`) instead of `String`
- [x] 4.3 Add tools `get_entity`, `get_related`, `get_evidence`, `get_context`, `get_timeline` returning `Json<T>`; parse `EntityRef` from string param
- [x] 4.4 Map query errors to `rmcp::ErrorData`; add `kb-query` dep to `kb-mcp`
- [x] 4.5 `cargo check -p kb-mcp`

## 5. CLI parity

- [x] 5.1 Add subcommands `kb entity <ref>`, `kb related <ref> [--kind] [--limit]`, `kb evidence <record-id>`, `kb context <ref> [--limit]`, `kb timeline <ref> [--limit]`
- [x] 5.2 Print human-readable output with a `--json` flag dumping the typed payload
- [x] 5.3 Add `kb-query` dep to `kb-cli`; `cargo check -p kb-cli`

## 6. Verify

- [x] 6.1 `rustfmt --edition 2021` on new files; edit existing files to match surrounding style
- [x] 6.2 `cargo check --workspace`
- [x] 6.3 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 6.4 `cargo test --workspace`
- [x] 6.5 `openspec validate structured-evidence-api --strict`

Note: as with `typed-evidence-graph`, the repo is not `cargo fmt`-clean at
baseline (compact hand formatting; CI runs build/clippy/test, not
`cargo fmt --check`). New files were formatted with `rustfmt --edition 2021`;
existing files were edited to match surrounding style rather than reformatted,
to keep the diff minimal. `kb-query`/`kb-mcp` use `schemars = "1"` (with
`chrono04`) to match the schemars version rmcp 1.7 requires for structured
`Json<T>` output.
