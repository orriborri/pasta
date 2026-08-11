# Implementation Plan

> **Reconciliation note (2026-08-11).** Tasks **1–10** are complete — implemented
> via the working-tree drift and verified in-session (build green; task-7/8/8.1/9
> tests pass). This spec was written assuming `crates/tui` stays and only gets
> trimmed; we **deleted the whole TUI crate** in task 4, so every TUI-coupled item
> below is struck as moot. Two `VaultLayout` accessors (`agents`, `timetracking`)
> have **no consumer** once the TUI is gone and are struck to avoid re-introducing
> the dead surface this spec exists to remove. Task 15's counts are corrected to
> the post-deletion reality. Several drift-created "tests" only **grep source
> text** rather than exercise behavior (flagged inline) — they are not counted as
> real coverage.

- [x] 1. Close the concurrent-fetch window
  - Make `FETCH_GROUP` public in `crates/backend/src/scheduler.rs` (already staged)
  - Guard `Command::ForceFetch` in `commands.rs` with the `running_native` fetch-group marker
  - Flash "fetch already running" when a manual trigger is rejected — DONE (wording is `"fetch-cycle already running"`)
  - Clear the marker when the spawned cycle finishes, on both success and failure — DONE (`run` never returns `Err`; clear always runs)
  - Merge the completed-fetcher set instead of assigning it wholesale — DONE (merge happens inside `fetch_cycle::run`, both scheduler and command handler)
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [ ] 1.1 Write a test proving the fetch cycle is single-flight
  - Insert the fetch-group marker into a test `AppState`, dispatch `Command::ForceFetch`, assert no cycle is spawned and a rejection message is sent
  - Assert the marker is cleared after a cycle completes
  - ⚠ A file exists (`tests/fetch_cycle_single_flight.rs`) but it **greps `commands.rs` as text** for `running_native` — it does not dispatch `ForceFetch`. Replace with a behavioral test.
  - _Requirements: 2.1, 2.2, 2.4_

- [x] 2. Delete dead crates [deletion]
  - Delete `crates/search/` and its `Cargo.toml` workspace member entry
  - Delete `crates/mcp/` and its `Cargo.toml` workspace member entry
  - Verify no remaining references: `grep -r "pasta-search\|pasta-mcp" --include=*.toml --include=*.rs --include=*.sh .`
  - _Requirements: 1.1, 1.7_

- [x] 3. Delete dead tests and unread outputs [deletion]
  - Delete `crates/backend/tests/wiki_e2e.rs` and the empty `crates/common/tests/` directory
  - Delete `crates/kb-storage/src/work_items.rs`, its `build_work_items` call site in `crates/kb-cli/src/main.rs`, and the `work_items.parquet` write path
  - Delete the inert `~/.kb/sync_state.db` and `~/.kb/sync_state.json`, and the stray `~` directory in the repo root — NOTE: repo `~` dir removed; the `~/.kb/sync_state.*` files live outside the repo and were left untouched
  - _Requirements: 1.2, 1.3, 1.7_

- [x] 4. Delete unused public API [deletion]
  - Remove `vault::load_inbox`, `vault::suggest_columns`, `vault::today_daily_exists`, `data::RunEntry`, `data::history_path`, `data::load_history`
  - Inline `load_waiting` into `load_people` if that is its only remaining caller, otherwise keep it — KEPT (still called in `vault.rs`)
  - ~~Remove TUI `Mode::Adding`, `InputField`, `handle_input_key`, `advance_input`, `ui::draw_input_dialog`~~ — SUPERSEDED: the entire `crates/tui` crate was deleted, so these items are gone with it
  - Remove `ipc::Event::SyncProgress` and its TUI handler — DONE (event removed; handler gone with the TUI crate)
  - _Requirements: 1.4, 1.5, 1.6, 1.7, 1.8_

- [x] 5. Align specs with the code
  - Delete `openspec/specs/roadmap-health/` and `openspec/specs/relevance-feedback/`, leaving their change proposals in `openspec/changes/`
  - Rewrite `openspec/specs/state-persistence/spec.md` to drop requirements naming `_sync_state.json`, `index_new_files`, `run_sync`, `history/sync_linear.rs`
  - Rewrite `openspec/specs/search-filtering/spec.md` to describe over-fetch-then-filter and drop deleted module names
  - Correct the cosine-threshold claim in `openspec/specs/semantic-routing/spec.md`
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [x] 6. Align documentation and config with the code
  - Remove `[schedules.kb-sync]`, `[kb] sources`, and `[kb] sync_on_fetch` from `KB_README.md`
  - Either wire `[kb] sources` to the fetch cycle's hardcoded source list or delete both fields from `crates/common/src/config.rs`; record which and why — DECIDED: **deleted** both fields (sources are hardcoded per call site; the two hardcoded lists diverge, so wiring is out of scope for a cleanup)
  - _Requirements: 3.6, 3.7_

- [x] 7. Cover the frontmatter helpers before changing them
  - Tests for `set_frontmatter` (upsert existing, insert inside the fence, body preserved), `remove_frontmatter`, `column_tag`, `set_column_tag` (non-column tags survive), and `parse_task` (`done` and `rejected` filtered out)
  - Test the no-frontmatter case returns input unchanged
  - DONE: 9 characterization tests in `crates/common/src/vault.rs`, all passing
  - _Requirements: 8.1_

- [x] 8. Make the task set singular
  - Change `vault::Task` to carry the path it was loaded from, replacing the bare `file` field
  - Make `vault::load_tasks` enumerate recursively through `walk_md_files`
  - Update `rewrite_status`, `close_task`, `reopen_task`, `approve_task`, `reject_task`, `update_task_priority` to write the loaded path
  - ~~Update the TUI editor-open path~~ — moot (TUI crate deleted); the Trello sync uses the path field (`load_task_files` walks recursively, writes `TaskFile.path`)
  - _Requirements: 4.1, 4.2, 4.4, 4.5, 4.6_

- [x] 8.1 Write tests for subfolder task handling
  - Create a task in a nested directory, assert `load_tasks` finds it
  - Close and approve it, assert the nested file changed and no top-level file was created
  - Assert task count is unchanged across a simulated routing move
  - DONE: 4 tests in `crates/common/src/vault.rs`, all passing
  - _Requirements: 8.2, 4.3_

- [x] 9. Resolve the kb data directory once
  - Move the `KbConfig` resolution out of `backend/src/kb_search.rs` into one shared constructor honouring `[kb] data_dir` — DONE: single site `pasta_common::config::kb_config()` (delegates to pure `kb_config_for`)
  - Replace all six `KbConfig::default()` call sites (`kb-sync` ×2, `kb-cli`, `kb-mcp`, readers) with that constructor — DONE (kb-mcp gained a `pasta-common` dep; no cycle)
  - Verify with a scratch `data_dir`: ingestion writes there and search reads from there — DONE via unit test `kb_config_tests` (empty→default, scratch→honoured)
  - _Requirements: 5.6, 5.7, 5.8_

- [x] 10. Remove the kb subprocess boundary
  - Replace `Command::new("kb").arg("sync")` in `commands.rs` `Backfill` with an in-process `kb_sync` call that reports real success or failure — DONE (`kb_sync::run` → Ok/Err flashes)
  - Replace `Command::new("kb")` in `inbox.rs` with a direct `ParquetStore` read plus date filter — DONE
  - Verify `--process-inbox` end to end with `kb` absent from `PATH` — verified structurally (no `kb` subprocess remains); NOT run live to avoid mutating the real vault
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] 10.1 Write a test that backfill failure is reported as failure
  - Assert a failing sync produces an error message to the client, not a success flash
  - ⚠ `tests/backfill.rs` exists but is unaudited — confirm it exercises the failure→error mapping rather than grepping source
  - _Requirements: 5.4_

- [ ] 11. Revert the launcher workaround [deletion]
  - Remove the `kb-cli` build step and `target/release` `PATH` export from `start.sh` and `run-sync.sh` — REAL, still pending: `start.sh` still runs `cargo build … -p kb-cli` and `export PATH="$PWD/target/release:$PATH"` with a now-false comment ("daemon shells out to kb"); task 10 removed that subprocess. (`run-sync.sh` is already clean.)
  - ~~Confirm `start.sh` still starts the daemon and TUI~~ — moot: no TUI. `start.sh` already `exec`s the daemon with no interactive UI; just confirm it still starts the daemon.
  - _Requirements: 5.9_

- [ ] 12. Introduce VaultLayout
  - Add `VaultLayout` to `pasta_common::vault` with resolved paths for tasks, inbox, daily notes, archive, stale archive, PARA folders, people, roadmap, feeds, triage patterns, weekly meetings, ~~timetracking~~, ~~agents~~ — MOSTLY DONE: `VaultLayout` exists and most accessors are wired. STRIKE `timetracking()` and `agents()` — they have **no consumer** after the TUI deletion (dead surface). Remove them unless a live caller is found.
  - Log a warning naming the directory when a layout path is missing, instead of returning an empty listing — implemented; but see 12.1
  - _Requirements: 6.1, 6.3_

- [x] 12.1 Write tests for layout resolution and missing-directory warnings
  - Assert each accessor composes the configured vault path — DONE (`vault_layout_*_path` tests)
  - Assert a missing directory produces a warning rather than a silent empty result — DONE: `warn_if_missing` now returns whether it emitted a (deduplicated) warning; the two former `panic!("TODO")` stubs are real tests (missing→warns, repeat→suppressed). No stderr capture needed; `cargo test --workspace` passes.
  - _Requirements: 6.1, 6.3_

- [x] 13. Replace path literals in common and backend
  - `vault_manager.rs` already routed through `VaultLayout` (drift). Remaining literals lifted into new accessors: `VaultLayout::waiting()` (`Waiting For.md`), `stale_archive()` (`4. Archive/Tasks-Stale`), and fixed the dead+wrong `roadmap()` (was `0. Inbox/roadmap`; now top-level `Roadmap/`, the real dir and cli.rs's actual target — it had no other callers)
  - Consumers updated: `vault.rs::load_waiting`→`waiting()`; `vault_manager.rs` ×2 `Tasks-Stale`→`stale_archive()`; `cli.rs::route_tasks`→`roadmap()`; `trello::linked_card_ids` `["Tasks","Archive"]`→`[tasks(), archive()]` — the latter also fixes a latent bug (it scanned a non-existent top-level `Archive/` rather than `4. Archive/`, so archived tasks were never excluded from Trello re-adoption)
  - `vault_organize.rs` already uses `projects()/areas()`; `inbox.rs` uses `tasks()/inbox()`; the `"Tasks"` in `vault_organize` is a category label, not a path
  - FLAG (semantic, not a literal — out of scope): `weekly_meetings()` = `0. Inbox/Weekly Meetings` but a top-level `Meetings/` dir exists; same class as the roadmap bug, left for a decision
  - _Requirements: 6.1, 6.2_

- [x] 14. Replace path literals in kb crates
  - ~~`crates/tui` daily note, timetracking, and agents directories~~ — moot (crate deleted)
  - `kb-sync`'s `FEEDS_DIR`: already done by the drift — `feeds_dir()` uses `VaultLayout::new(vault_path).feeds()` (covered by `write_feeds_uses_vault_layout_for_feeds_dir`)
  - `kb-fetchers/src/vault.rs` PARA literals: DONE — `VaultFetcher::fetch` now builds a `VaultLayout` and routes Tasks/People/Projects/Areas/Resources through `tasks()/people()/projects()/areas()/resources()`; added `pasta-common` dep to kb-fetchers (no cycle: `pasta-common → kb-core` only)
  - FLAG: `kb-pipeline` (`registry.rs`, `entity_manager.rs`) still has `People`/`1. Projects` literals and no `pasta-common` dep — out of task-14 scope; fold into task 15 or a follow-up
  - _Requirements: 6.2, 6.4_

- [ ] 15. Deduplicate shadowed constants
  - Collapse the ~~four~~ **three** `COLUMN_TAGS` definitions (canonical `vault::COLUMN_TAGS`, the inline copy in `vault_manager.rs`, and the Trello `COLUMNS` list-name mapping); the fourth was in a deleted crate
  - Collapse the fetch-group source lists: **four** copies of `["gitlab","linear","slack","gmail","calendar"]` (`commands.rs`, `config.rs`, `server.rs`, `scheduler.rs`) into one const; and make `commands.rs` backfill use the existing `kb_sync::ALL_SOURCES` instead of its inline duplicate
  - Grep to confirm the vault literals appear only in `VaultLayout`
  - _Requirements: 6.2, 6.5, 6.6_

- [ ] 16. Cover the pure parsers left untested
  - Tests for `extract_work_item_key`, `extract_mr_from_name`, `parse_mr_url`
  - Tests for `acp::parse_notification` covering both `session/update` and legacy `session/notification` formats
  - _Requirements: 8.3, 8.4_

- [ ] 17. Add the CI gate
  - Workflow running `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`
  - ⚠ PARTIALLY BLOCKED: `cargo test --workspace` now passes (12.1 fixed); `cargo clippy --workspace -- -D warnings` still fails on pre-existing debt — land the clippy cleanup (13/15) before enabling `-D warnings`. `tests/ci_workflow.rs` stub exists.
  - _Requirements: 8.5_

- [ ] 18. Gather storage evidence
  - Record which code paths query Tantivy, which query LanceDB, and which read Parquet directly
  - Add `vault` to the daemon's fetch sources, or record why not — NOTE: the **scheduled** fetch names omit `vault` (`["gitlab","linear","slack","gmail","calendar"]`), though `kb_sync::ALL_SOURCES` and the manual backfill include it — so scheduled runs may leave semantic routing/PARA audit inert. Prerequisite for judging semantic search.
  - Measure latency and result quality for identical queries via Tantivy-only versus the hybrid path on the real corpus
  - Measure the cost of the `read_all` full-corpus reads across the Parquet file set
  - `tests/storage_evidence.rs` stub exists (audit whether it measures anything or just compiles)
  - _Requirements: 7.1, 7.2, 7.3, 7.4_

- [ ] 19. Open the storage follow-up
  - Write up findings and create a follow-up spec carrying a recommendation
  - `tests/storage_followup.rs` stub exists (audit)
  - _Requirements: 7.5_
