## Why

Three generations of architecture coexist in the tree: the retired `history/` LanceDB index, the standalone `kb` engine invoked as a subprocess, and the current in-process `kb-sync` library. Each migration added code without removing the previous layer, so the repo now carries ~1,100 lines that nothing calls, two promoted specs with zero implementation, two dead `Command::new("kb")` call sites that report success while doing nothing, and two incompatible notions of "the set of tasks" — one of which silently hides tasks from the user. None of this is bad code; there is simply more of it than the problem needs, and every future change pays interest on it.

## What Changes

- Delete code with no callers: `crates/search` (opens a store deleted in the history migration), `crates/mcp` (never registered anywhere), `tests/wiki_e2e.rs` (drives the deleted architecture and writes into the live vault), `work_items.rs` (written, never read), unreachable TUI `Mode::Adding` dialog, and unused `pasta_common` public API (`load_inbox`, `load_waiting`, `suggest_columns`, `today_daily_exists`, `data::history_*`)
- Delete `openspec/specs/roadmap-health/` and `openspec/specs/relevance-feedback/` — promoted main specs with zero implementing code. Their change proposals stay in `openspec/changes/` as unstarted work
- Guard `Command::ForceFetch` with the scheduler's `running_native` marker so the manual fetch key cannot run a second concurrent fetch/index cycle over the same Parquet/Tantivy/LanceDB/SQLite stores
- Make the task set singular: `vault::load_tasks` walks `Tasks/` recursively so initiative-routed tasks stay visible, and task mutation resolves files by their real path rather than rebuilding `Tasks/<file>`
- **BREAKING** (internal): collapse the kb boundary to library-only — remove both `Command::new("kb")` sites (`commands.rs` `Backfill`, `inbox.rs` inbox processing), replacing them with the `kb_sync` calls the daemon already makes elsewhere. `crates/kb-cli` remains a standalone user-facing CLI but stops being part of the daemon's runtime contract
- Resolve `[kb] data_dir` in exactly one place that readers and writers share, so a non-default value cannot point the daemon's search at a store nothing writes
- Introduce a `VaultLayout` type owning every vault-relative path, replacing ~20 `"Tasks"` literals, 5 copies of the daily-note path, and the `.feeds` directory that is vault-relative in `common` and an absolute personal path in `kb-sync`
- Out of scope: consolidating the four storage engines. That needs evidence this change produces (which indexes the daemon actually queries), and lands as a separate change

## Capabilities

### New Capabilities
- `vault-layout`: one typed source of truth for every vault-relative path, so a folder rename fails loudly in one place instead of silently returning empty directory reads in twenty
- `task-visibility`: every task file under `Tasks/`, at any depth, is visible through the task set and mutable through it

### Modified Capabilities
- `kb-api`: the daemon consumes kb as a library only; `kb` is no longer spawned as a subprocess, and the data directory resolves once for readers and writers alike
- `search-filtering`: a single search entry point remains after `crates/search` and `crates/mcp` are deleted; requirements referencing deleted modules are removed
- `state-persistence`: requirements naming deleted files (`_sync_state.json`, `history/sync_*.rs`, `run_sync`) are removed, and the fetch cycle gains a single-flight requirement covering both the scheduler and the manual trigger

## Impact

- `crates/search/`, `crates/mcp/` — deleted; `Cargo.toml` workspace members drop two entries and the `lancedb`/`arrow-array` edges they pulled in
- `crates/backend/tests/wiki_e2e.rs` — deleted (currently the only integration test, and it mutates the real vault)
- `crates/backend/src/commands.rs` — `ForceFetch` single-flight guard; `Backfill` calls `kb_sync` in-process and reports real success or failure
- `crates/backend/src/inbox.rs` — reads records via `kb_sync`/`ParquetStore` instead of `kb recent`
- `crates/backend/src/kb_search.rs` — becomes the single `KbConfig` resolution point, used by writers too
- `crates/common/src/vault.rs` — `load_tasks` recursive, `Task` carries a real path, dead API removed, path literals move to `VaultLayout`
- `crates/common/src/config.rs`, `crates/kb-sync/src/lib.rs` — `FEEDS_DIR` absolute path replaced by layout lookup
- `crates/tui/` — `Mode::Adding` and its dialog removed; task list now shows subfolder tasks
- `openspec/specs/` — two spec directories deleted, three rewritten to match the code
- `KB_README.md` — drops the `[schedules.kb-sync]`, `sources`, and `sync_on_fetch` documentation for config keys nothing reads
- No user-visible feature is removed. Task counts in the TUI will increase, because previously hidden routed tasks become visible again
