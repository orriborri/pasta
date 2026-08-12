# Implementation Plan

> **Reconciliation note (2026-08-12).** This spec was written assuming the ACP
> **TUI chat is preserved** while the scheduled agentic layer is removed. Since
> then, `remove-accreted-architecture` deleted the TUI crate and then the
> orphaned ACP chat surface entirely (`acp.rs`, `Command::{ChatStart,ChatPrompt,
> ChatStop}`, `Event::Chat`, `ChatEventType`, `ConnectionState.acp` — committed
> `42f8fd1`). So **Requirement 2 and task 2 are VOID** (there is no chat path to
> preserve or test), and task 1's "confirm acp is not a scheduled caller" clause
> is moot. The spec's *core* — removing `spawn_agent`/`run_agentic_schedules`/
> `[[agents]]` and retiring `generate_weekly` — is unaffected and still to do;
> all those symbols are confirmed still present in the code.

- [ ] 0. Confirm the base is clean [prerequisite]
  - Verify `remove-accreted-architecture` has landed: working tree clean, `cargo build --workspace` and `cargo test --workspace` green on `main`
  - Do NOT start the deletions below on a dirty tree — abort and report if the base is not clean
  - _Requirements: 5.1_

- [ ] 1. Map every call site of the scheduled agentic layer
  - `grep -rn "spawn_agent\|run_agentic_schedules" crates` and record each caller
  - (The "confirm `acp::spawn`/`send_prompt` are not among them" check is moot — the ACP chat was removed in `remove-accreted-architecture`; those symbols no longer exist.)
  - _Requirements: 1.1, 1.2, 1.5_

- [x] 2. ~~Cover the ACP chat path before touching anything around it~~ — VOID: the ACP TUI chat (`acp.rs`, `ChatStart`/`ChatPrompt`/`ChatStop`, `Event::Chat`, `ChatEventType`) was deleted in `remove-accreted-architecture` (commit `42f8fd1`) once the TUI left. There is no chat path to cover, and Requirement 2 no longer applies.

- [ ] 3. Remove `run_agentic_schedules` from the scheduler
  - Delete `run_agentic_schedules` in `crates/backend/src/scheduler.rs`
  - Change `scheduler::spawn` to invoke only `run_native_schedules`
  - Remove the now-unused `use crate::process::{cleanup_finished, spawn_agent}` import
  - _Requirements: 1.2_

- [ ] 4. Delete `spawn_agent` and its process-tracking [deletion]
  - Delete `process.rs::spawn_agent` and `cleanup_finished` if it only served scheduled agents
  - Remove `state.rs` process-tracking fields that exist only for scheduled agents (`running`/`completed` maps) and the `Event::AgentStarted`/`AgentFinished` variants + their handlers
  - Remove the manual agent-run command in `commands.rs` (the `spawn_agent` call at the manual dispatch site) and its `ipc::Command` variant
  - _Requirements: 1.1, 1.5, 1.6_

- [ ] 5. Remove the `[[agents]]` config surface [deletion]
  - Delete the `agents` field, the `Agent` struct, `depends_on`, and `agent_timeout_minutes` from `crates/common/src/config.rs`
  - Remove the `[[agents]]` blocks from `/home/orre/.kiro/pasta.toml`
  - _Requirements: 1.3, 1.4_

- [ ] 5.1 Verify the daemon still parses config
  - Load config with `[[agents]]` removed; assert no parse error and the daemon starts
  - _Requirements: 1.4_

- [ ] 6. Retire `generate_weekly` [deletion]
  - Delete `vault_organize::generate_weekly`
  - Remove the `--weekly` CLI command in `main.rs`/`cli.rs` and any `commands.rs` wiring
  - Grep to confirm no remaining `Meetings/Weekly/<date>-summary.md` generation in pasta
  - Also drop `VaultLayout::weekly_meetings()`: `generate_weekly` is its only consumer, so it becomes dead surface here — this resolves the discrepancy flagged in `remove-accreted-architecture` task 13 (accessor pointed at `0. Inbox/Weekly Meetings` while the real dir is top-level `Meetings/`), since the accessor is deleted rather than repointed
  - _Requirements: 4.1, 4.2, 4.4_

- [ ] 7. Assert the deterministic writers are untouched
  - Confirm `inbox::process`/`create_task`, `generate_daily`, `repair_links`, `audit_para` compile and their existing tests pass unchanged
  - Confirm `fetch_cycle.rs` still calls `generate_daily()` post-fetch
  - Confirm `--daily`, `--organize`, `--route-tasks` behave as before
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 8. Full-workspace gate
  - `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace` all green
  - Grep confirms `spawn_agent`, `run_agentic_schedules`, `[[agents]]`, and `generate_weekly` no longer appear anywhere
  - _Requirements: 5.1, 5.2, 5.3_
