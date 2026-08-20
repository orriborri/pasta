# Implementation Plan

This plan intentionally introduces the graph in phases. Do not start by wiring
Graphify into production. Establish pasta's own stable domain/retrieval boundary
first, then evaluate Graphify behind it.

- [ ] 0. Establish baseline and capture current retrieval behaviour [prerequisite]
  - Run `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace`
  - Record a small fixture set of real queries and current `kb search` results/latency for later comparison
  - Confirm Parquet remains the documented source of truth and identify every direct caller of `kb_storage::hybrid_search`
  - _Requirements: 4.1, 5.5, 10.6, 10.7_

- [ ] 1. Add the entity domain model to `kb-core`
  - Add `EntityId`, `Entity`, `EntityKind`, and `SourceRef` or reuse/extend the existing source-reference type where appropriate
  - Implement deterministic namespaced ID helpers for Linear issues, GitHub repositories/PRs, vault objects, people, and code symbols
  - Add serialization round-trip tests and collision/determinism tests
  - Do not modify ingestion or search behaviour in this task
  - _Requirements: 1.1, 1.2, 1.3, 1.5, 10.1_

- [ ] 2. Add relations and provenance to `kb-core`
  - Add `Relation`, `RelationKind`, and provenance variants Explicit/Extracted/Inferred
  - Require evidence references for Explicit/Extracted relations and evidence + model + confidence for Inferred relations
  - Add tests for relation identity/deduplication keys and provenance serialization
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 3.4_

- [ ] 3. Define `GraphRepository` without selecting a graph database
  - Add a provider-neutral repository contract for entity upsert, relation upsert, entity lookup, bounded related traversal, and derived-data rebuild
  - Define graph schema/version metadata
  - Define invariants for duplicate edges and dangling edges
  - Add an in-memory implementation for contract tests first
  - _Requirements: 2.3, 2.4, 2.5, 9.1, 9.3_

- [ ] 4. Implement the first persistent graph repository in `kb-storage`
  - Measure expected entity/edge counts from the current corpus before choosing storage
  - Prefer the smallest existing mechanism that satisfies the contract; SQLite tables are acceptable for the prototype, but keep graph data clearly separate from sync cursor/state tables
  - Add indexes required for `from`, `to`, kind-filtered, and depth-1 traversal
  - Record corpus size, traversal latency, rebuild time, and operational notes in `docs/` or the spec reconciliation notes
  - _Requirements: 2.4, 2.5, 9.4, 9.5_

- [ ] 5. Add graph rebuild support
  - Extend reindex/rebuild plumbing so derived entities and relations can be regenerated without mutating Parquet
  - Detect graph schema version mismatch and require/recommend rebuild
  - Validate no dangling edges and no duplicate deterministic edges after rebuild
  - Keep graph rebuild failure isolated from the authoritative raw corpus
  - _Requirements: 9.1, 9.2, 9.3, 10.2_

- [ ] 6. Introduce `KnowledgeService` as the retrieval boundary
  - Define storage-neutral `SearchQuery`, `KnowledgeResult`, `RelatedQuery`, and `EntityContext` types
  - Route existing hybrid search through `KnowledgeService::search` with unchanged RRF behaviour first
  - Add `entity`, `related`, and `context` operations backed by `GraphRepository`
  - Migrate pasta/backend, kb-cli, and kb-mcp callers away from direct storage-specific retrieval where practical
  - Grep to ensure no new direct Tantivy/LanceDB query code appears outside `kb-storage`
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 10.2_

- [ ] 7. Add a source-independent extraction contract
  - Define an extractor interface that accepts canonical knowledge records and emits zero or more entities/relations
  - Extraction failure for one record must log/report and leave raw persistence + text/vector indexing successful
  - Keep source-specific API/network code inside fetchers, not graph storage
  - Add a no-op/default extractor proving new sources remain searchable without graph support
  - _Requirements: 1.4, 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 8. Implement deterministic Linear relationships
  - Create Issue entities from stable Linear issue identifiers
  - Create Project/team entities only where stable source metadata exists
  - Emit explicit/extracted `belongs_to` or equivalent relations with source evidence
  - Add fixtures proving repeated sync produces stable IDs and no duplicate edges
  - _Requirements: 1.2, 2.3, 3.1, 3.2, 10.3_

- [ ] 9. Implement deterministic GitHub relationships
  - Create Repository and PullRequest entities from stable GitHub identity
  - Emit PullRequest → Repository relation
  - Where a PR/body/metadata explicitly names a known issue key/link, emit an evidence-backed `implements` relation; do not infer from vague text in this phase
  - Add tests for multiple repositories containing the same PR number
  - _Requirements: 1.2, 2.2, 3.1, 3.2, 10.3_

- [ ] 10. Implement deterministic Vault/task relationships
  - Create Task/Project entities from stable vault paths/frontmatter using `VaultLayout`
  - Emit task → project/initiative relations from explicit path/frontmatter information
  - Ensure routed tasks keep stable identity when their authoritative identity can be preserved; document the chosen identity rule where moves necessarily change it
  - Add subfolder fixtures matching the recursive task semantics from `remove-accreted-architecture`
  - _Requirements: 1.2, 2.2, 3.1, 7.2, 10.3_

- [ ] 11. Add bounded graph-aware retrieval
  - Identify entities associated with hybrid-search results and use them as graph seeds
  - Default expansion depth to 1 with configurable hard fan-out/result limits
  - Annotate every returned result with retrieval reason: lexical, semantic, direct relation, or graph path
  - Preserve a hybrid-only mode/fallback
  - Add tests proving cycles cannot cause unbounded traversal
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 12. Build a retrieval evaluation fixture
  - Use the baseline queries captured in task 0 plus relationship-heavy questions such as "what implemented issue X?", "what discussions relate to project Y?", and "what changed around component Z?"
  - Compare hybrid-only vs hybrid-plus-graph result relevance and latency on the same corpus
  - Record misses, harmful graph expansions, and useful expansions; tune limits/ranking only from evidence
  - _Requirements: 5.5, 10.4_

- [ ] 13. Extend `kb-mcp` with entity-oriented tools
  - Add `search_knowledge`, `get_entity`, `find_related`, and `get_context` (names may follow MCP naming conventions but semantics must match)
  - Return clear not-found responses for unknown entity IDs
  - Make provenance/evidence available in relation responses
  - Route all tools through `KnowledgeService`
  - _Requirements: 3.5, 8.1, 8.2, 8.3, 8.4, 10.4_

- [ ] 14. Add CLI diagnostics over the same service
  - Add `kb entity`, `kb related`, and `kb context` only as thin diagnostic/user interfaces over `KnowledgeService`
  - Do not duplicate graph traversal or ranking logic in `kb-cli`
  - Keep existing `kb search` behaviour compatible
  - _Requirements: 8.5, 10.7_

- [ ] 15. Define the `CodeGraphProvider` adapter
  - Add provider-neutral repository indexing/import contract outside Graphify-specific code
  - Define translation rules from provider nodes/edges to `CodeSymbol` entities and `calls`/`imports`/`implements` relations
  - Add a disabled/no-provider implementation proving the rest of kb-engine works without a code graph
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [ ] 16. Build an optional Graphify proof of concept [experiment]
  - Integrate Graphify only through `CodeGraphProvider`; do not expose its native types from `kb-core`
  - Index the pasta repository and import code symbols/relations into the derived graph
  - Treat Graphify absence/failure as degradation of code-structure enrichment, not failure of Slack/Gmail/Linear/Vault search
  - Keep the existing Tree-sitter path intact during this task
  - _Requirements: 6.2, 6.3, 6.4, 6.7_

- [ ] 17. Benchmark Graphify against the existing Tree-sitter path [decision gate]
  - Run both against the same pasta revision/repositories
  - Compare language/symbol coverage, call/import edge correctness, incremental latency, full rebuild time, real coding-query usefulness, operational complexity, and failure behaviour
  - Record concrete examples of edges each provider gets right/wrong
  - Write an explicit ADOPT GRAPHIFY or REJECT GRAPHIFY recommendation
  - Do not remove Tree-sitter or make Graphify mandatory in this task
  - _Requirements: 6.5, 6.6, 6.7, 10.5_

- [ ] 18. Open the follow-up code-graph consolidation change
  - If Graphify is adopted, specify deletion of overlapping Tree-sitter code-graph extraction and define Graphify freshness/rebuild operations
  - If Graphify is rejected, specify which existing parser improvements are justified by the benchmark
  - In either case, production must end with one authoritative provider for each deterministic code relationship
  - _Requirements: 6.5, 6.6, 10.5_

- [ ] 19. Full-workspace and architecture gate
  - Run `cargo build --workspace`
  - Run `cargo clippy --workspace -- -D warnings`
  - Run `cargo test --workspace`
  - Verify `kb search` still works without graph expansion and without Graphify installed
  - Verify Parquet is still authoritative and graph data can be rebuilt
  - Verify pasta/backend and MCP clients use the knowledge boundary rather than Graphify/Tantivy/LanceDB directly
  - _Requirements: 4.5, 6.2, 9.1, 10.6, 10.7_
