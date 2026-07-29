## Context

After `kb-engine-architecture`, the `kb-*` crates own ingestion, Parquet storage, and hybrid (vector + full-text) search over a single `~/.kb` store. Pasta's `backend/src/history/` still maintains a parallel LanceDB index and sync loop, and also hosts pasta-specific vault logic (`vault_organize`, `symbols`, `chunker`) that was never knowledge-base indexing. The two indexes now overlap and drift. Task 6.5 of the prior change ("remove `history/`") was deferred because a wholesale delete would break backend search, semantic task routing, and daily-note generation, which still call into `history/`.

## Goals / Non-Goals

**Goals:**
- Single search/index backend: pasta delegates all search to kb-engine hybrid search.
- Retire pasta's duplicate LanceDB/`.history` index and its sync modules.
- Preserve pasta-specific vault features (daily note, organize, semantic routing) by relocating them out of `history/`.
- Leave the build green and features working at every step; delete only after rewiring is verified.

**Non-Goals:**
- Changing kb-engine's search ranking or storage.
- Changing the daily-note format or vault layout.
- Removing the lightweight feed fetchers that write `.feeds/`.

## Decisions

### 1. Search delegates to kb-engine via library call
Backend depends on `kb-core` + `kb-storage` and calls `kb_storage::hybrid_search` directly (in-process, same workspace) rather than shelling out. `Command::Search` and `vault_manager` semantic routing both use it, mapping results back to pasta's `SearchResultItem`. This removes `history::indexer::search` without losing filtering behavior.

### 2. Sync delegates to `kb sync`
The `history-sync` schedule, `--backfill`, and `--sync-once` are repointed at `kb sync` (already wired as the `kb-sync` schedule). `--migrate`/`--reindex` (which operated on the old JSON→LanceDB index) are removed; `kb reindex` is the replacement.

### 3. Relocate, don't delete, pasta-specific vault logic
`vault_organize`, `symbols`, and `chunker` move to a new `backend/src/vault/` module tree. They are vault upkeep, not KB indexing, so they outlive `history/`. Import paths update accordingly.

### 4. Stage the removal behind verification
Order: (a) add kb-backed search + relocate vault logic; (b) rewire all callers; (c) build + smoke-test search and daily note; (d) only then delete the dead `history/` ingestion modules and drop unused deps (`lancedb`, `arrow-*`, tree-sitter) from backend if nothing else uses them.

## Risks / Trade-offs

- **Search regression**: kb-engine's `~/.kb` must be populated before old index is removed; mitigate by running `kb sync` and verifying results before deleting `history/indexer`.
- **Hidden coupling**: `vault_organize` may reach into `indexer`/`embedder` for related-doc lookups; those call sites must be repointed to kb search during relocation.
- **Dependency removal**: dropping `lancedb`/tree-sitter from backend is only safe if `symbols` relocation fully severs the dependency; otherwise keep them.
- **CLI breakage**: removing `--migrate`/`--reindex` is breaking for any scripts; documented in the proposal.
