# Implementation Plan

> ⚠️ **GATED SPEC — DO NOT EXECUTE YET.** Every deletion task below is blocked
> behind the gate in Requirement 1. Ralph and any executor MUST NOT check off or
> run tasks 3+ until tasks 0–2 are complete and signed off by the user. Deleting
> pasta's deterministic writers before KiroCrew parity is proven, or while the
> KiroCrew gateway is unstable, is the exact failure this gate prevents.

- [ ] 0. GATE: KiroCrew gateway stability confirmed [precondition — human sign-off required]
  - Confirm no loopstall-watchdog bounces over a sustained observation window
  - User explicitly confirms Phase 2 may proceed
  - _Requirements: 1.1, 1.5_

- [ ] 1. Build the KiroCrew task-creation replacement and prove parity [precondition]
  - Implement a zero-token KiroCrew `script=` cron reproducing `create_task`:
    deterministic `kb-<slug>.md` filename, early-return-if-exists, and
    `task_match::classify` rules + approval gate
  - Run it in shadow against real inbox records; diff its output against pasta's
    `--route-tasks` output until identical
  - _Requirements: 1.2, 1.3, 1.4, 3.1_

- [ ] 2. Build the KiroCrew daily-feed replacement and prove parity [precondition]
  - Implement a deterministic materializer for the `## 🔗 Activity Feed` splice
    (zero-token), optionally with a subagent enrichment layer on top
  - Verify in-place splice preserves surrounding daily-note content
  - Decide and record the home for `repair_links`/`audit_para` (KiroCrew cron or
    conscious retirement)
  - _Requirements: 1.2, 2.1, 3.3_

- [ ] 3. Cut over task creation and delete pasta's writer [BLOCKED until 0–2] [deletion]
  - Switch to the KiroCrew cron; remove `inbox.rs::process`/`create_task` and the
    `--route-tasks`/`--process-inbox` paths
  - _Requirements: 3.2, 4.1_

- [ ] 4. Cut over the daily feed and delete `generate_daily` [BLOCKED until 0–2] [deletion]
  - Remove `vault_organize::generate_daily`, its `fetch_cycle.rs` call, and the
    `--daily` CLI command
  - _Requirements: 2.2, 2.3_

- [ ] 5. Remove the rest of `vault_organize` [BLOCKED until 0–2] [deletion]
  - Once `repair_links`/`audit_para` have their decided home, delete
    `crates/backend/src/vault_organize.rs` and the `--organize` command
  - _Requirements: 3.3, 4.1_

- [ ] 6. Prove pasta is read-only [BLOCKED until 3–5] [verification]
  - Grep confirms no `fs::write`/vault-mutation path remains in pasta
  - Confirm the MCP read-API, fetchers, pipeline, storage, and hybrid search are
    unchanged
  - `cargo build/clippy/test --workspace` green
  - _Requirements: 4.2, 4.3, 4.4_
