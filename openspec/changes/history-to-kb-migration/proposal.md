## Why

The `kb-engine-architecture` change moved ingestion, storage, and hybrid search into the standalone `kb-*` crates, but pasta's `backend/src/history/` modules still own the daemon's search index, sync loop, and several vault features. The two now overlap (both embed and index the same sources), and the duplicate index drifts. The final step of that migration — removing `history/` — was deferred because parts of it have no kb-engine home yet. This change does that migration safely and deletes the dead code.

## What Changes

- Rewire `Command::Search` (backend) and `fetchers/vault_manager` semantic routing to call kb-engine hybrid search instead of `history::indexer::search`.
- Replace the `history-sync` schedule, `--backfill`, and `--sync-once` indexing paths with `kb sync` (kb-engine owns ingestion + indexing).
- Relocate pasta-specific vault logic that currently lives under `history/` — `vault_organize` (daily note, index_vault, repair_links, audit_para), `symbols`, and `chunker` — into a non-`history` module, since it is not knowledge-base indexing and has no kb-engine equivalent.
- Remove the now-dead `history/` ingestion + indexing modules: `indexer`, `embedder`, `feed_history`, `state`, `sync_slack`, `sync_gmail`, `sync_linear`, `sync_git_log`, `sync_wiki`, `sync_repos`, `config`. **BREAKING** for the `--migrate`/`--reindex` CLI flags that operated on pasta's old LanceDB index.
- Keep the lightweight feed fetchers (`fetchers/*` that write `.feeds/`) — those remain pasta's responsibility.

## Capabilities

### New Capabilities
- `vault-organization`: Daily-note generation and vault upkeep (index, repair links, PARA audit) as a first-class pasta capability, decoupled from knowledge-base indexing.

### Modified Capabilities
- `search-filtering`: Backend and semantic-routing search SHALL delegate to kb-engine hybrid search rather than pasta's internal history index.

## Impact

- Code: `backend/src/{commands,fetch_cycle,scheduler,cli,main}.rs`, `backend/src/fetchers/vault_manager.rs`, deletion of `backend/src/history/` (minus relocated modules).
- CLI: `--migrate`/`--reindex` removed or repointed at `kb reindex`; `--backfill`/`--sync-once` delegate to `kb sync`.
- Data: pasta's old LanceDB/`.history` index is retired; kb-engine's `~/.kb` store becomes the single index.
- Dependencies: backend can drop `lancedb`, `arrow-*`, tree-sitter deps once `symbols` is relocated and the internal index is gone.
- Risk: search and daily-note generation must be verified end-to-end after rewiring before `history/` is deleted.
