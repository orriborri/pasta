# Implementation Plan

This plan intentionally improves pasta's architectural boundaries before adding
new graph infrastructure. The first half should leave the system better even if
the graph/Graphify experiment is later rejected.

The governing boundary is:

> Fetchers observe. Pipeline interprets. Storage persists. KnowledgeService
> answers. Pasta acts.

- [ ] 0. Establish the baseline and dependency map [prerequisite]
  - Run `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace`
  - Record representative `kb search` queries, results, and latency for later comparison
  - Identify every caller of `kb_storage::hybrid_search`, `TextIndex`, `VectorStore`, and direct Parquet reads
  - Map current side effects in `kb-sync::index`, including People/vault writes, discovery surfacing, storage writes, and embedding
  - Record every current meaning of "entity": `Record.entities`, `EntityManager`, registry entries, and any other entity-like types
  - _Requirements: 4.1, 5.5, 10.6, 10.7_

- [ ] 1. Introduce `KnowledgeService` before adding graph behavior [architecture]
  - Add a service/application boundary (`kb-service` crate or an equivalently explicit module after measuring whether a new crate is justified)
  - Define storage-neutral `SearchQuery` and `KnowledgeResult` types
  - Implement `KnowledgeService::search` by delegating to the current RRF hybrid search without changing ranking
  - Route `kb-mcp`, `kb-cli`, and pasta backend search through `KnowledgeService`
  - Remove direct Tantivy/LanceDB construction from MCP/application layers
  - Add an architecture test/grep gate so new direct search-index access stays inside the knowledge infrastructure layer
  - _Requirements: 4.1, 4.2, 4.5, 4.6, 10.2_

- [ ] 2. Make retrieval failures observable instead of silently empty [reliability]
  - Replace `unwrap_or_default` degradation in hybrid retrieval with explicit retriever status/error handling
  - Introduce provider/retriever abstractions for lexical and semantic retrieval without changing their implementations
  - Return successful partial results only when degradation is explicit and observable
  - Include retriever health/status in diagnostics and make MCP capable of reporting degraded retrieval
  - Preserve the existing RRF ranking as the baseline strategy
  - _Requirements: 4.1, 4.4, 5.5, 10.2_

- [ ] 3. Clarify the existing entity terminology before adding the graph model [deconfliction]
  - Rename/refactor `kb_pipeline::entity_manager::EntityManager` to reflect its real role as a People/vault projector (for example `PeopleProjector`)
  - Ensure this component does not become the canonical knowledge-entity store
  - Document the responsibility of `EntityRegistry` and decide whether it is identity normalization, enrichment configuration, or another projection concern
  - Inventory and document the current semantics of `Record.entities: Vec<String>`
  - Do not add graph-domain `Entity` types until these names and responsibilities are unambiguous
  - _Requirements: 1.1, 1.4, 7.1, 10.1_

- [ ] 4. Replace overloaded record fields with structured source metadata [domain cleanup]
  - Design a backward-compatible evolution of `Record` that separates common fields from source-specific metadata
  - Stop relying on positional `tags` semantics such as Linear status/project encoded by vector position
  - Introduce typed metadata for high-value sources first (Linear, GitHub/Git, Slack, Vault); migrate incrementally
  - Introduce typed actor/source references where practical instead of overloading `author`, `participants`, and `thread_id`
  - Preserve Parquet compatibility through an explicit schema/version migration plan
  - Add serialization and compatibility fixtures for old and new record representations
  - _Requirements: 1.3, 7.1, 7.2, 9.1, 10.1_

- [ ] 5. Split pure pipeline interpretation from persistence and vault side effects [architecture]
  - Refactor the pipeline to return a `ProcessedBatch` (or equivalent) rather than performing external writes inside interpretation stages
  - `ProcessedBatch` should carry transformed records plus discoveries/entity candidates/other derived outputs needed downstream
  - Move People-file updates and discovery surfacing behind explicit projectors executed by orchestration after the pure pipeline completes
  - Keep Parquet, text-index, vector-index, and future graph writes outside pure transformation stages
  - Add tests proving pipeline output is deterministic and does not mutate the vault/filesystem
  - _Requirements: 7.1, 7.3, 7.4, 9.1, 10.2_

- [ ] 6. Define ingestion/indexing sinks explicitly [architecture]
  - Separate raw/canonical record persistence, lexical indexing, semantic indexing, graph projection, and vault projection into explicit sink/repository interfaces or clearly isolated functions
  - Refactor `kb-sync::index` into orchestration over those components instead of implementing all behavior directly
  - Define failure policy for each sink: authoritative-write failure vs derived-index degradation vs optional projection failure
  - Ensure a failed optional projector cannot corrupt or suppress authoritative record persistence
  - Add integration tests for partial failure behavior
  - _Requirements: 7.3, 7.4, 9.1, 9.2, 10.2_

- [ ] 7. Add the canonical entity domain model to `kb-core`
  - Add `EntityId`, `Entity`, `EntityKind`, and `SourceRef` or reuse/extend an existing source-reference type
  - Implement deterministic namespaced IDs for Linear issues, repositories/PRs, vault objects, people, and code symbols
  - Keep cross-source objects separate by default; connect/alias them only with deterministic evidence
  - Add serialization round-trip, determinism, and collision tests
  - Do not make vault filenames the canonical identity mechanism
  - _Requirements: 1.1, 1.2, 1.3, 1.5, 10.1_

- [ ] 8. Add relation and universal provenance types to `kb-core`
  - Add `Relation`, `RelationKind`, and Explicit/Extracted/Inferred provenance
  - Require evidence references for explicit/extracted relations and evidence + model + confidence for inferred relations
  - Define reusable retrieval evidence/reason types so provenance is available for search results as well as graph edges
  - Add tests for relation identity/deduplication and provenance serialization
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 3.4_

- [ ] 9. Add source-independent entity/relation extractor contracts
  - Define deterministic extractor interfaces accepting canonical processed records and returning entities/relations
  - Keep source API/network behavior in fetchers rather than extractors or graph storage
  - Isolate extractor failure so raw persistence and fuzzy search remain usable
  - Add a no-op/default extractor proving sources remain searchable without graph support
  - Integrate extraction into `ProcessedBatch` rather than creating another side-effecting pipeline path
  - _Requirements: 1.4, 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 10. Define `GraphRepository` without choosing a graph database
  - Add a provider-neutral contract for entity upsert, relation upsert, entity lookup, bounded traversal, and derived rebuild
  - Define graph schema/version metadata plus duplicate-edge and dangling-edge invariants
  - Add an in-memory implementation for contract tests
  - Keep the contract in the knowledge/service boundary and storage implementation in `kb-storage`
  - _Requirements: 2.3, 2.4, 2.5, 9.1, 9.3_

- [ ] 11. Implement the smallest persistent graph repository [measurement gate]
  - Measure expected entity/edge counts and query patterns before selecting storage
  - Prefer existing infrastructure such as dedicated SQLite graph tables if it meets measured needs
  - Keep graph data clearly separate from sync cursor/state tables even when the same SQLite engine is used
  - Add indexes for from/to/kind and depth-1 traversal
  - Record corpus size, traversal latency, rebuild time, and operational complexity
  - Do not introduce a dedicated graph database without evidence
  - _Requirements: 2.4, 2.5, 9.4, 9.5_

- [ ] 12. Add deterministic high-value relationships
  - Linear: Issue → Project/Team using stable explicit source metadata
  - GitHub/Git provider data: PullRequest → Repository and explicit issue-link → `implements`
  - Vault: Task → Project/Initiative from explicit path/frontmatter information
  - People: Record/Message → Person from explicit normalized actor identity
  - Add repeated-sync fixtures proving stable IDs and no duplicate deterministic edges
  - Avoid broad LLM inference in this phase
  - _Requirements: 1.2, 2.2, 3.1, 3.2, 7.2, 10.3_

- [ ] 13. Add graph rebuild and derived-data validation
  - Rebuild entities/relations from authoritative records and configured external deterministic providers
  - Detect graph schema-version mismatches
  - Validate dangling-edge and duplicate-edge invariants after rebuild
  - Ensure rebuild never mutates Parquet authoritative data
  - Keep inferred edges separately identifiable/versioned if inference is introduced later
  - _Requirements: 9.1, 9.2, 9.3, 10.2_

- [ ] 14. Evolve retrieval from one RRF function into explicit retrieval signals
  - Keep current RRF as `RankingStrategy::Rrf` (or equivalent) baseline
  - Represent lexical rank, semantic similarity, direct relation, graph path, and later recency as explicit retrieval signals
  - Ensure direct deterministic relationships can outrank weak fuzzy matches when the query is relationship-oriented
  - Attach retrieval reasons/evidence to returned results
  - Keep ranking strategy independently testable and replaceable
  - _Requirements: 4.3, 5.1, 5.2, 5.3, 5.5_

- [ ] 15. Add bounded graph-aware context retrieval
  - Extend `KnowledgeService` with `entity`, `related`, and `context`
  - Identify seed entities from fuzzy results and allow bounded graph expansion
  - Default graph depth to 1 with hard fan-out/result limits; require explicit request for deeper traversal initially
  - Preserve hybrid-only fallback/mode
  - Add cycle and high-degree-node tests proving traversal cannot explode
  - _Requirements: 4.2, 4.3, 4.4, 5.1, 5.2, 5.3, 5.4_

- [ ] 16. Make MCP agent-oriented and evidence-bearing
  - Route every MCP operation through `KnowledgeService`
  - Expose `search_knowledge`, `get_entity`, `find_related`, and `get_context` with storage-neutral schemas
  - Include evidence/provenance and retrieval reasons where useful
  - Return structured degraded-status information when a retriever/provider is unavailable
  - Add clear unknown/not-found behavior
  - _Requirements: 3.5, 8.1, 8.2, 8.3, 8.4, 10.4_

- [ ] 17. Keep CLI as a thin diagnostic interface
  - Route `kb search` through `KnowledgeService`
  - Add `kb entity`, `kb related`, and `kb context` only as thin service clients
  - Do not duplicate traversal/ranking logic in `kb-cli`
  - Keep existing search UX compatible where practical
  - _Requirements: 8.5, 10.7_

- [ ] 18. Treat the Obsidian vault as a projection boundary [architecture]
  - Identify remaining knowledge-layer code that writes directly into the vault
  - Move People/discovery and similar derived writes behind explicit `VaultProjector`-style interfaces owned by pasta/application orchestration
  - Keep task/PARA/workflow ownership in pasta while keeping knowledge identity independent from vault representation
  - Define idempotency rules and tests for projections
  - Record follow-up work for large remaining `vault_manager`/`vault_organize` responsibilities that should be split by capability
  - _Requirements: 7.3, 7.4, 10.2_

- [ ] 19. Clarify the pasta application crate boundary
  - Document `backend` as application/daemon orchestration rather than knowledge implementation
  - Evaluate renaming it to `pasta-daemon` or `pasta-app` in a separate low-risk change; do not mix a crate rename into graph implementation unless evidence shows it reduces migration risk
  - Ensure scheduler, tasks, inbox, vault projection, Trello, and workflow coordination remain application responsibilities
  - Ensure retrieval/index implementation lives behind knowledge-service contracts
  - _Requirements: 4.5, 10.2, 10.7_

- [ ] 20. Define `CodeGraphProvider` as an external knowledge-producer adapter
  - Place the provider contract at the service/integration boundary, not inside graph storage
  - Define repository indexing/import plus translation to canonical `CodeSymbol` entities and relations
  - Add a disabled/no-provider implementation proving all non-code knowledge remains functional
  - Ensure provider-native node/edge types never cross into `kb-core`
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [ ] 21. Build an optional Graphify proof of concept [experiment]
  - Integrate Graphify only through `CodeGraphProvider`
  - Index the pasta repository and translate its code symbols/edges into canonical entities/relations
  - Treat Graphify absence/failure/staleness as code-structure degradation, not KB failure
  - Keep the current Tree-sitter path intact during the experiment
  - _Requirements: 6.2, 6.3, 6.4, 6.7_

- [ ] 22. Benchmark Graphify against the existing Tree-sitter path [decision gate]
  - Compare the same repository revision and coding questions
  - Measure symbol/language coverage, call/import edge correctness, initial indexing, incremental update latency, query usefulness, operational complexity, and failure behavior
  - Record concrete false-positive/false-negative examples from both providers
  - Write an explicit ADOPT GRAPHIFY or REJECT GRAPHIFY recommendation
  - Do not remove either provider during the benchmark task
  - _Requirements: 6.5, 6.6, 6.7, 10.5_

- [ ] 23. Build a retrieval-quality evaluation suite [decision gate]
  - Reuse baseline queries from task 0 plus relationship-heavy questions such as `what implemented issue X?`, `what discussions relate to project Y?`, and `what changed around component Z?`
  - Compare hybrid-only, graph-aware, and code-graph-assisted retrieval on the same corpus
  - Measure relevance, latency, evidence quality, harmful expansion, and degraded-provider behavior
  - Tune ranking/fan-out only from recorded evidence
  - _Requirements: 5.5, 10.4, 10.5_

- [ ] 24. Open follow-up consolidation changes
  - If Graphify is adopted, specify deletion of overlapping Tree-sitter code-graph extraction and Graphify freshness/rebuild operations
  - If Graphify is rejected, specify only parser improvements justified by the benchmark
  - Review measured usage/benefit of Tantivy, LanceDB, Parquet full reads, and graph persistence before proposing any storage consolidation
  - Production must end with one authoritative deterministic provider per code relationship
  - _Requirements: 6.5, 6.6, 9.4, 9.5, 10.5_

- [ ] 25. Full workspace and architecture gate
  - Run `cargo build --workspace`
  - Run `cargo clippy --workspace -- -D warnings`
  - Run `cargo test --workspace`
  - Verify `kb search` works with graph expansion disabled and Graphify absent
  - Verify Parquet remains authoritative and every derived index/graph can be rebuilt
  - Verify MCP/CLI/pasta use `KnowledgeService` rather than Tantivy/LanceDB/Graphify directly
  - Verify pure pipeline tests perform no filesystem/vault writes
  - Verify degraded retrieval/provider failures are observable rather than silently converted to empty results
  - _Requirements: 4.5, 6.2, 9.1, 10.6, 10.7_
