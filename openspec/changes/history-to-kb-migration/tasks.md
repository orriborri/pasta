## 1. kb-backed search in backend

- [x] 1.1 Add `kb-core` + `kb-storage` as backend dependencies
- [x] 1.2 Add a `search` helper in backend that calls `kb_storage::hybrid_search` and maps results to `ipc::SearchResultItem` (source/participant/date filters pre-limit)
- [x] 1.3 Rewire `Command::Search` to use the kb-backed helper instead of `history::indexer::search`
- [x] 1.4 Rewire `fetchers/vault_manager` semantic routing search to the kb-backed helper

## 2. Prepare kb-engine to back vault organization

> Discovered during apply: the kb `VaultFetcher` only indexes `Tasks/`, `People/`,
> `1. Projects/`, and kb `SearchResult` carries no file path/category. `vault_organize`
> (repair_links/audit_para) needs both. These must land before vault_organize can be
> fully decoupled from the `.history` index.

- [x] 2.1 Extend `kb-fetchers` `VaultFetcher` to also index `2. Areas/` and `3. Resources/`
- [x] 2.2 Add `url` to kb `SearchResult` (store path in LanceDB schema + Tantivy; requires `kb reindex`) so category can be derived
- [x] 2.3 Expose path/category on the backend `kb_search` result mapping

## 3. Relocate and rewrite vault_organize (off .history)

- [x] 3.1 Move `vault_organize` out of `history/` into `backend/src/vault_organize.rs`; update `mod`/imports in `cli`, `commands`, `fetch_cycle`, `main`
- [x] 3.2 Repoint `repair_links`/`audit_para` related-doc lookups to `kb_search` (title = first content line; category from new path field)
- [x] 3.3 Rewrite `generate_daily` activity feed to query kb for recent records per source instead of reading `.history` feed dirs
- [x] 3.4 Drop `index_vault` (kb `VaultFetcher` + `kb sync` now own vault indexing); remove its calls in `cli`/`commands`

## 4. Delegate sync to kb-engine

- [x] 4.1 Repoint the `history-sync` schedule and `--backfill`/`--sync-once` to `kb sync`
- [x] 4.2 Remove `--migrate`/`--reindex` CLI flags (document `kb reindex` as the replacement)
- [x] 4.3 Replace `feed_history` writes in `fetch_cycle` with `kb sync` (feeds still written to `.feeds/`)

## 5. Verify before deletion

- [x] 5.1 Build the workspace (cargo build --workspace green); running `kb sync` to populate `~/.kb` requires the live env
- [ ] 5.2 Smoke-test backend search and semantic routing return kb-engine results
- [ ] 5.3 Smoke-test daily-note generation and vault organize from the relocated module

## 6. Remove dead history/ modules

- [x] 6.1 Delete `history/{indexer,embedder,feed_history,state,chunker,config,symbols}` and `history/sync_*`
- [x] 6.2 Remove `pub mod history;` and the `history::embedder::init()` call from `main`
- [x] 6.3 Drop now-unused backend deps (`lancedb`, `arrow-*`, tree-sitter)
- [x] 6.4 `cargo clippy` clean across the workspace; update `KB_README`/docs to note the single index
