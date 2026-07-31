## 1. Delete dead code and close the fetch race

- [ ] 1.1 Guard `Command::ForceFetch` in `crates/backend/src/commands.rs` with the scheduler's `running_native` fetch-group marker; flash "fetch already running" when rejected, and remove the marker when the spawned cycle finishes
- [ ] 1.2 Verify the guard: start a scheduled cycle, trigger the force-fetch key, confirm the log shows one cycle and the client shows it as running
- [ ] 1.3 Delete `crates/search/` and remove it from `Cargo.toml` workspace members
- [ ] 1.4 Delete `crates/mcp/` and remove it from `Cargo.toml` workspace members
- [ ] 1.5 Delete `crates/backend/tests/wiki_e2e.rs` and the now-empty `crates/common/tests/` directory
- [ ] 1.6 Delete `crates/kb-storage/src/work_items.rs`, its `build_work_items` caller in `crates/kb-cli/src/main.rs`, and the `work_items.parquet` write path
- [ ] 1.7 Delete unused `pasta_common` API: `vault::load_inbox`, `vault::load_waiting`, `vault::suggest_columns`, `vault::today_daily_exists`, `data::RunEntry`, `data::history_path`, `data::load_history` (keep `load_waiting` logic inlined into `load_people` if still needed there)
- [ ] 1.8 Delete unreachable TUI input dialog: `Mode::Adding`, `InputField`, `handle_input_key`, `advance_input`, `ui::draw_input_dialog`
- [ ] 1.9 Delete `ipc::Event::SyncProgress` and its TUI handler — nothing emits it
- [ ] 1.10 Delete the inert `~/.kb/sync_state.db` and `~/.kb/sync_state.json` leftovers, and the stray `~` directory in the repo root
- [ ] 1.11 `cargo build --workspace` and `cargo clippy --workspace` clean; commit as one deletion commit

## 2. Align specs and docs with the code

- [ ] 2.1 Delete `openspec/specs/roadmap-health/` (zero implementing code; `openspec/changes/roadmap-health-check/` stays as unstarted work)
- [ ] 2.2 Delete `openspec/specs/relevance-feedback/` (zero implementing code; `openspec/changes/search-relevance-feedback/` stays)
- [ ] 2.3 Update `KB_README.md`: remove `[schedules.kb-sync]`, `[kb] sources`, and `[kb] sync_on_fetch` — the schedule was removed and neither config key is read by any code
- [ ] 2.4 Delete the `[kb] sources` and `[kb] sync_on_fetch` fields from `crates/common/src/config.rs`, or wire them to `fetch_cycle`'s hardcoded source list — decide and record which
- [ ] 2.5 Correct `openspec/specs/semantic-routing/spec.md`: it describes a 0.7 cosine threshold that does not exist in `route_new_tasks_semantic`
- [ ] 2.6 Commit specs and docs separately from code

## 3. Make the task set singular

- [ ] 3.1 Add unit tests for the frontmatter helpers before touching them: `set_frontmatter`, `remove_frontmatter`, `column_tag`, `set_column_tag`, `parse_task` (pure functions, no vault access)
- [ ] 3.2 Change `vault::Task` to carry the path it was loaded from, replacing the bare `file` name field
- [ ] 3.3 Make `vault::load_tasks` enumerate recursively via `walk_md_files`, so it and the Trello sync share one enumeration
- [ ] 3.4 Update `rewrite_status`, `close_task`, `reopen_task`, `approve_task`, `reject_task`, `update_task_priority` to write the task's real path instead of rebuilding `<task dir>/<file>`
- [ ] 3.5 Update the TUI's editor-open path and Trello sync to use the path field
- [ ] 3.6 Add a test proving a task in a subfolder is loaded, closed, and approved at its real location
- [ ] 3.7 Verify against the live vault: count tasks before and after a routing pass and confirm the count is unchanged

## 4. Collapse the kb boundary to library-only

- [ ] 4.1 Move the `KbConfig` resolution from `backend/src/kb_search.rs` into a single shared constructor that honours `[kb] data_dir`
- [ ] 4.2 Replace all six `KbConfig::default()` call sites (`kb-sync` ×2, `kb-cli`, `kb-mcp`, and readers) with the shared constructor
- [ ] 4.3 Verify: set `[kb] data_dir` to a scratch path, run a fetch, confirm ingestion writes there and search reads from there
- [ ] 4.4 Replace `Command::new("kb").arg("sync")` in `commands.rs` `Backfill` with an in-process `kb_sync` call that reports real success or failure instead of unconditionally flashing success
- [ ] 4.5 Replace `Command::new("kb")` in `inbox.rs` with a direct `ParquetStore` read plus date filter, dropping the nonexistent `--json` flag entirely
- [ ] 4.6 Verify `--process-inbox` end to end with `kb` absent from `PATH`, and confirm pending tasks are created when `[kb.task_rules]` matches
- [ ] 4.7 Revert the `start.sh` / `run-sync.sh` `PATH` export and `kb-cli` build step added to work around the dead boundary — no longer needed

## 5. Centralise vault paths

- [ ] 5.1 Add `VaultLayout` to `pasta_common::vault`: resolved paths for tasks, inbox, daily notes, archive, stale archive, PARA folders, people, roadmap, feeds, triage patterns, weekly meetings, timetracking, agents
- [ ] 5.2 Log a warning naming the directory when a layout path is missing, instead of returning an empty `read_dir`
- [ ] 5.3 Replace literals in `crates/common` and `crates/backend/src/fetchers/vault_manager.rs` (~14 sites)
- [ ] 5.4 Replace literals in `crates/backend/src/vault_organize.rs`, `cli.rs`, `inbox.rs`, `trello/mod.rs`
- [ ] 5.5 Replace literals in `crates/tui` (daily note, timetracking, agents dirs)
- [ ] 5.6 Replace `kb-sync`'s hardcoded absolute `FEEDS_DIR` with the layout lookup, and `kb-fetchers/vault.rs`'s PARA literals
- [ ] 5.7 Deduplicate the constants that shadow each other: `COLUMN_TAGS` (4 copies incl. Trello list names) and the fetch-group source list (6 copies)
- [ ] 5.8 Grep for remaining `"Tasks"`, `"0. Inbox"`, `"4. Archive"`, `"1. Projects"` literals; assert only `VaultLayout` contains them

## 6. Gather evidence for the storage decision

- [ ] 6.1 Record what each store actually serves: which code paths query Tantivy, which query LanceDB, and which read Parquet directly
- [ ] 6.2 Add `vault` to the daemon's fetch sources (or document why not), so PARA audit and semantic routing stop being silently inert — this is the prerequisite for judging semantic search
- [ ] 6.3 Measure search latency and result quality for the same queries through Tantivy-only versus the hybrid path, on the real 1,072-record corpus
- [ ] 6.4 Measure the cost of the full-corpus `read_all` calls (170 Parquet files) used by inbox processing and the PARA audit
- [ ] 6.5 Write up the findings and open a follow-up change for storage consolidation, with a recommendation

## 7. Guard against regression

- [ ] 7.1 Add unit tests for the pure functions this change touches but does not rewrite: `extract_work_item_key`, `extract_mr_from_name`, `parse_mr_url`, `acp::parse_notification`
- [ ] 7.2 Add a CI workflow running `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` — the clippy config exists but nothing enforces it
- [ ] 7.3 Run `openspec archive remove-accreted-architecture` once phases 1–5 are complete and phase 6 has produced its follow-up change
