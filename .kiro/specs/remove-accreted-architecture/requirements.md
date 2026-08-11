# Requirements Document

## Introduction

pasta carries three coexisting generations of architecture: the retired `history/` LanceDB index, the standalone `kb` engine invoked as a subprocess, and the current in-process `kb-sync` library. Each migration landed as addition without deletion, leaving ~1,100 lines nothing calls, two promoted specs with zero implementing code, two dead `Command::new("kb")` call sites that report success while doing nothing, and two incompatible notions of "the set of tasks" — one of which silently hides tasks from the user.

This spec removes the accreted layers and tightens three seams. It changes no user-visible feature. It deliberately does not consolidate the four storage engines; it produces the evidence needed to decide that separately.

## Glossary

- **Accreted layer**: code, spec, or documentation serving an architecture that has been replaced but not removed
- **Task set**: the collection of task files the daemon exposes to clients via the state snapshot
- **Routed task**: a task file moved by vault maintenance from `Tasks/` into `Tasks/<Initiative>/`
- **kb boundary**: the interface between the pasta daemon and the knowledge-base engine, currently both a library dependency and a subprocess invocation
- **Single-flight**: an operation that permits at most one concurrent execution regardless of trigger source
- **Vault layout**: the set of vault-relative paths (`Tasks`, `0. Inbox`, `4. Archive`, PARA folders, etc.)

## Requirements

### Requirement 1

**User Story:** As the maintainer, I want code with no callers deleted, so that reading the codebase does not require distinguishing live code from abandoned code.

#### Acceptance Criteria

1. WHEN the workspace is built THEN `crates/search` and `crates/mcp` SHALL NOT exist and SHALL NOT appear in `Cargo.toml` workspace members
2. WHEN the test suite is collected THEN `crates/backend/tests/wiki_e2e.rs` SHALL NOT exist, because it imports no crate under test and writes into the live vault
3. WHEN `kb-storage` is compiled THEN `work_items.rs` and its `work_items.parquet` write path SHALL NOT exist, because nothing reads that output
4. WHEN `pasta_common` is compiled THEN `vault::load_inbox`, `vault::suggest_columns`, `vault::today_daily_exists`, `data::RunEntry`, `data::history_path`, and `data::load_history` SHALL NOT exist
5. WHEN the TUI is compiled THEN `Mode::Adding`, `InputField`, `handle_input_key`, and `draw_input_dialog` SHALL NOT exist, because no code transitions into that mode
6. WHEN the IPC types are compiled THEN `Event::SyncProgress` SHALL NOT exist, because nothing emits it
7. WHEN deletion is complete THEN `cargo build --workspace` and `cargo clippy --workspace` SHALL report no new errors or warnings
8. WHEN deletion is complete THEN no user-visible feature SHALL be removed

### Requirement 2

**User Story:** As a user pressing the force-fetch key, I want the daemon to refuse a second concurrent fetch, so that four storage engines are not written by two cycles at once.

#### Acceptance Criteria

1. WHEN a fetch cycle is already running AND the user triggers a manual fetch THEN the daemon SHALL NOT start a second cycle
2. WHEN a manual fetch is rejected for that reason THEN the daemon SHALL send the client a message stating a cycle is already running
3. WHEN a manually triggered cycle is running THEN the state snapshot SHALL report it as running, so the user is not led to trigger it again
4. WHEN a manually triggered cycle finishes THEN the running marker SHALL be cleared, whether the cycle succeeded or failed
5. WHEN a fetch cycle completes THEN the completed-fetcher set SHALL be merged rather than replaced wholesale, preserving records written during the cycle

### Requirement 3

**User Story:** As the maintainer, I want the spec directory to describe the system that exists, so that specs are usable as truth rather than as history.

#### Acceptance Criteria

1. WHEN the spec set is reviewed THEN `openspec/specs/roadmap-health/` and `openspec/specs/relevance-feedback/` SHALL NOT exist, because no implementing code exists for either
2. WHERE a change proposal for removed specs exists in `openspec/changes/` THEN it SHALL remain, marked as unstarted work
3. WHEN `openspec/specs/state-persistence/spec.md` is read THEN it SHALL NOT contain requirements naming `_sync_state.json`, `index_new_files`, `run_sync`, or `history/sync_linear.rs`
4. WHEN `openspec/specs/search-filtering/spec.md` is read THEN it SHALL describe the over-fetch-then-filter behaviour the code implements, and SHALL NOT name deleted modules
5. WHEN `openspec/specs/semantic-routing/spec.md` is read THEN it SHALL NOT claim a cosine similarity threshold that `route_new_tasks_semantic` does not apply
6. WHEN `KB_README.md` is read THEN it SHALL NOT document `[schedules.kb-sync]`, `[kb] sources`, or `[kb] sync_on_fetch`
7. WHEN `[kb] sources` and `[kb] sync_on_fetch` are examined THEN they SHALL either be read by code or removed from the config struct

### Requirement 4

**User Story:** As a user, I want every task I have to appear in the task list, so that tasks do not silently disappear when the vault is reorganised.

#### Acceptance Criteria

1. WHEN a task file exists at any depth under the task directory THEN it SHALL appear in the task set
2. WHEN vault maintenance routes a task into an initiative subfolder THEN the task SHALL remain visible in the TUI and in the state snapshot
3. WHEN the routing pass moves N tasks THEN the task count in the state snapshot SHALL be unchanged
4. WHEN a task is closed, approved, rejected, reopened, or reprioritised THEN the write SHALL target the path the task was loaded from
5. WHEN a task in a subfolder is mutated THEN no file SHALL be created at the top level of the task directory
6. WHEN the state snapshot, the Trello sync, and vault maintenance enumerate tasks THEN they SHALL use one shared enumeration function

### Requirement 5

**User Story:** As a user, I want the daemon to work without an installed `kb` binary, so that a missing executable cannot silently disable features.

#### Acceptance Criteria

1. WHEN the daemon runs THEN it SHALL NOT spawn `kb` as a subprocess
2. WHEN `kb` is absent from `PATH` THEN fetch, index, search, and inbox processing SHALL all work
3. WHEN the user triggers a backfill THEN the daemon SHALL call the kb library in-process and report the real outcome, succeeding or failing visibly
4. WHEN a backfill fails THEN the client SHALL NOT be told it succeeded
5. WHEN inbox processing runs THEN it SHALL read recent records through the kb library rather than parsing subprocess stdout
6. WHEN `[kb] data_dir` is set to a non-default path THEN ingestion SHALL write to that path AND search SHALL read from that same path
7. WHEN `[kb] data_dir` is unset THEN every component SHALL use the same default
8. WHEN the workspace is searched for `KbConfig` construction THEN exactly one resolution site SHALL exist
9. WHERE `start.sh` and `run-sync.sh` build `kb-cli` and prepend `target/release` to `PATH` as a workaround THEN those additions SHALL be reverted once the subprocess sites are gone

### Requirement 6

**User Story:** As the maintainer, I want vault paths defined once, so that renaming a folder fails loudly in one place instead of silently returning empty directory listings in twenty.

#### Acceptance Criteria

1. WHEN a crate needs a vault-relative path THEN it SHALL obtain it from a single `VaultLayout` type
2. WHEN the workspace is searched THEN the literals `"Tasks"`, `"0. Inbox"`, `"4. Archive"`, `"1. Projects"`, `"2. Areas"`, and `"3. Resources"` SHALL appear only in `VaultLayout`
3. WHEN a layout directory is missing THEN the system SHALL log a warning naming that directory rather than treating it as empty
4. WHEN the feeds directory is resolved in any crate, including `kb-sync` THEN it SHALL be relative to the configured vault path and SHALL NOT be a hardcoded absolute path
5. WHEN column tags are referenced THEN one definition SHALL serve the Kanban board, the triage patterns, and the Trello list mapping
6. WHEN the fetch-group source names are referenced THEN one definition SHALL serve the scheduler, the snapshot builder, the command handler, and the TUI

### Requirement 7

**User Story:** As the maintainer, I want measured evidence about the storage layers, so that consolidating them is a decision rather than a guess.

#### Acceptance Criteria

1. WHEN the evidence phase completes THEN a written record SHALL state which code paths query Tantivy, which query LanceDB, and which read Parquet directly
2. WHEN semantic search is evaluated THEN the `vault` source SHALL be indexed by the daemon, or the reason for excluding it SHALL be recorded — otherwise the PARA audit and semantic routing remain silently inert
3. WHEN retrieval quality is compared THEN latency and results SHALL be measured for the same queries through the Tantivy-only path and the hybrid path on the real corpus
4. WHEN full-corpus reads are measured THEN the cost of `read_all` across the Parquet file set SHALL be recorded
5. WHEN the evidence is complete THEN a follow-up change SHALL be opened carrying a recommendation

### Requirement 8

**User Story:** As the maintainer, I want the pure functions this work touches covered by tests and a CI gate, so that the next refactor has a safety net.

#### Acceptance Criteria

1. WHEN the frontmatter helpers are modified THEN tests SHALL already cover `set_frontmatter`, `remove_frontmatter`, `column_tag`, `set_column_tag`, and `parse_task`
2. WHEN task path handling changes THEN a test SHALL prove a task in a subfolder is loaded, closed, and approved at its real location
3. WHEN the URL parsers are reviewed THEN tests SHALL cover `extract_work_item_key`, `extract_mr_from_name`, and `parse_mr_url`
4. WHEN the ACP wire format is reviewed THEN tests SHALL cover `parse_notification` for both message formats it accepts
5. WHEN a change is pushed THEN CI SHALL run `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace`
