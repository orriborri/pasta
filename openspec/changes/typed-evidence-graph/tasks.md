## 1. Canonical entity + relation types in kb-core

- [x] 1.1 Add `crates/kb-core/src/entity.rs`: `EntityKind` enum and `EntityRef { kind, id, project }` with `Display`/`FromStr` canonical form (`kind:id` and `kind:id@project`)
- [x] 1.2 Add `crates/kb-core/src/relation.rs`: `RelationKind`, `Derivation`, `Relation { subject, predicate, object, evidence_record_id, derivation, created_at }` and a stable `Relation::key()` (SHA-256 over identifying fields)
- [x] 1.3 Re-export `EntityRef`, `EntityKind`, `Relation`, `RelationKind`, `Derivation` from `kb-core/src/lib.rs`
- [x] 1.4 Unit tests: `EntityRef` round-trip (with/without project), invalid parse errors, `Relation::key()` stability and evidence-non-empty invariant
- [x] 1.5 `cargo check -p kb-core` + tests green

## 2. Graph store (derived index) in kb-storage

- [x] 2.1 Add `KbConfig::graph_path()` (`data_dir/graph.db`) in `crates/kb-core/src/config.rs`
- [x] 2.2 Add `crates/kb-storage/src/graph_store.rs`: `GraphStore` opening `graph.db`, creating `entities`/`relations` tables + indexes
- [x] 2.3 Implement `GraphStore::rebuild(&[Record], &[Relation])` (single transaction: clear, insert entities from records + relation endpoints, insert relations)
- [x] 2.4 Implement `GraphStore::upsert(&[Relation])` with `INSERT OR REPLACE` keyed on relation key (inserting missing endpoint entities)
- [x] 2.5 Implement read helpers: `relations_by_subject`, `relations_by_object`, `relations_by_evidence`, `entity_count`, `relation_count`
- [x] 2.6 Re-export `GraphStore` from `kb-storage/src/lib.rs`
- [x] 2.7 Unit tests against a temp `graph.db`: rebuild then counts; endpoint-without-record stored; upsert idempotency

## 3. Deterministic relation extractor in kb-pipeline

- [x] 3.1 Add `crates/kb-pipeline/src/relation_extract.rs` with pure `relations_from_records(&[Record]) -> Vec<Relation>`
- [x] 3.2 Implement rules: `AuthoredBy` (explicit author), `ParticipatedIn` (explicit participants), `PartOfThread` (explicit thread_id), `Mentions`/`References` (parsed `EntityRef` from `Record.entities`, rule-named per kind, `References` when subject kind is commit/MR)
- [x] 3.3 Parse `Record.entities` strings into canonical `EntityRef` (reuse `EntityRef::from_str`; keep a mapping for existing `linear:`/`mr:` mention forms)
- [x] 3.4 Sort output by `Relation::key()`; take all timestamps from records (no clock reads, no RNG)
- [x] 3.5 Re-export `relations_from_records` from `kb-pipeline/src/lib.rs`
- [x] 3.6 Unit tests: determinism (two runs equal), every relation's evidence id present in input, explicit-vs-rule derivation, no relation between merely-similar records

## 4. Wire reindex + incremental sync

- [x] 4.1 In `kb-cli` `cmd_reindex`: after reading Parquet, delete/rebuild `graph.db` via `relations_from_records` + `GraphStore::rebuild`
- [x] 4.2 In `kb-cli` `cmd_reprocess`: rebuild `graph.db` from the post-pipeline records
- [x] 4.3 In `kb-sync::index`: after Parquet/Tantivy/LanceDB writes, derive relations for the batch and `GraphStore::upsert` (log-and-continue on failure, matching existing derived-write handling)
- [x] 4.4 `cargo check --workspace`

## 5. Integration tests + verification

- [x] 5.1 Integration test: build a fixed `Vec<Record>` fixture, `GraphStore::rebuild`, assert known entity/relation counts and that a specific evidence-backed relation exists
- [x] 5.2 Integration test: rebuild twice from the same fixture → identical graph contents (determinism/rebuild invariant)
- [x] 5.3 Integration test: a relation whose object entity has no backing record is still stored, with evidence pointing at the mentioning record
- [x] 5.4 `cargo fmt` (new files), `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all green
- [x] 5.5 `openspec validate typed-evidence-graph --strict`

Note: the repo is not `cargo fmt`-clean at baseline (it uses compact hand
formatting; CI runs build/clippy/test, not `cargo fmt --check`). New files were
formatted with `rustfmt --edition 2021`; existing files were edited to match the
surrounding style rather than reformatted, to keep the diff minimal.
