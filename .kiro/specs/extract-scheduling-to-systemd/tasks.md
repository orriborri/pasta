# Implementation Plan

- [ ] 0. Confirm the sequencing gate before starting anything else
  - Read `.ralph/state.json`; confirm `spec` is no longer `remove-accreted-architecture` or that its `work_queue` is fully completed
  - Confirm `cargo build --workspace` and `cargo clippy --workspace` are clean on the branch this spec starts from
  - Do not proceed to task 1 until both are true
  - _Requirements: Non-Negotiable Sequencing Constraint_

- [ ] 1. Decouple daily-note generation from the fetch cycle
  - Remove the `generate_daily()` call from `crates/backend/src/fetch_cycle.rs::run`
  - Update the doc comment on `fetch_cycle::run` to drop the now-false "and generate the daily note" claim
  - _Requirements: 1.1, 1.2_

- [ ] 1.1 Write a test proving fetch and daily-note generation are independent
  - Assert `fetch_cycle::run` no longer calls into `vault_organize` (e.g. via a fake vault dir where `generate_daily` would visibly write a file, asserting it does not)
  - _Requirements: 1.1_

- [ ] 2. Add the two missing CLI entrypoints
  - Add `--vault-maintenance` to `main.rs`'s flag dispatch, calling `fetchers::vault_manager::run()` and exiting 0/1 based on its result
  - Add `--run-agent <name>` to `main.rs`'s flag dispatch: parse the next arg as the agent name, look it up in `config::get().agents`, error and exit non-zero if not found, otherwise call `process::spawn_agent` with its `prompt`/`cwd`, block on the returned `JoinHandle`, and exit non-zero if the agent timed out or failed to spawn
  - _Requirements: 1.3, 1.4, 1.5, 1.6, 1.7_

- [ ] 2.1 Write tests for the new flags
  - Test `--run-agent` with an unknown name exits non-zero without spawning anything
  - Test `--run-agent` with a known name resolves the correct prompt/cwd from a test config
  - _Requirements: 1.5, 1.6_

- [ ] 3. Remove the internal scheduler from daemon mode (commented, not deleted)
  - Comment out the `scheduler::spawn(state.clone())` call in `main.rs`'s daemon-mode branch, with a comment pointing at this spec and noting it is superseded by systemd units pending verification
  - Confirm daemon mode still binds the IPC socket and serves `server::run` with the scheduler disabled
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 4. Write the `pasta-backend.service` unit [unit-file]
  - `Type=simple`, `ExecStart=` the built `pasta-backend` binary with no flags, `Restart=on-failure`, `WantedBy=default.target`
  - Move `run-sync.sh`'s `MemoryMax=28G`/`MemoryHigh=20G` into the unit's `[Service]` section
  - Verify: `systemctl --user daemon-reload && systemctl --user enable --now pasta-backend.service` starts the daemon; `systemctl --user status pasta-backend.service` shows active; killing the process gets it restarted automatically
  - _Requirements: 4.1, 4.2_

- [ ] 5. Write the fetch timer/service pair and its two OnSuccess-chained services [unit-file]
  - `pasta-fetch.timer` (`OnCalendar=hourly`, `Persistent=true`) → `pasta-fetch.service` (`Type=oneshot`, execs `pasta-backend --fetch`)
  - `pasta-daily.service` (oneshot, execs `--daily`), triggered by `OnSuccess=` on `pasta-fetch.service`
  - `pasta-inbox-processor.service` (oneshot, execs `--run-agent inbox-processor`), triggered by `OnSuccess=` on `pasta-fetch.service`
  - Verify: `systemctl --user start pasta-fetch.service` and confirm both dependent units start afterward, in `journalctl --user`
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 6. Write the daily-writer chain [unit-file]
  - `pasta-daily-writer.service` (oneshot, execs `--run-agent daily-writer`), triggered by `OnSuccess=` on `pasta-inbox-processor.service`
  - Verify: triggering `pasta-inbox-processor.service` manually results in `pasta-daily-writer.service` starting afterward
  - _Requirements: 3.4_

- [ ] 7. Write the vault-maintenance, weekly, and trello timer/service pairs [unit-file]
  - `pasta-vault-maintenance.timer` (`OnCalendar=daily`, `Persistent=true`) → `pasta-vault-maintenance.service` (execs `--vault-maintenance`)
  - `pasta-weekly.timer` (`OnCalendar=Fri 15:00`, `Persistent=true`) → `pasta-weekly.service` (execs `--weekly`)
  - `pasta-trello.timer`/`.service` on the interval from `[schedules.trello]`, execs `--sync-trello` (which already self-gates on `[trello] enabled`)
  - Verify each with a manual `systemctl --user start`
  - _Requirements: 3.5, 3.6, 3.7_

- [ ] 8. Verify failure visibility and lingering
  - Induce a failure in one oneshot job (e.g. temporarily point `--vault-maintenance` at a nonexistent vault path) and confirm `systemctl --user status`/`journalctl --user -u` shows the failure without needing `pasta/logs/`
  - Run `loginctl show-user $(whoami) --property=Linger`; if not enabled, run `loginctl enable-linger $(whoami)` and record that this is a one-time host-level step outside the repo
  - _Requirements: 3.8, 4.4_

- [ ] 9. Update the launcher scripts
  - Replace `start.sh`'s pkill-and-relaunch with `systemctl --user restart pasta-backend.service` (keeping the build-if-stale step, or moving it into an `ExecStartPre=` on the unit — decide and record which)
  - Update or retire `run-sync.sh` accordingly, since its `systemd-run --user --scope` was a transient stand-in for what `pasta-backend.service` now does persistently
  - _Requirements: 4.5_

- [ ] 10. Run one full verification cycle and get explicit sign-off before deletion
  - Let fetch, daily, inbox-processor, daily-writer, vault-maintenance, weekly, and trello each run at least once via their real timers (not just manual `systemctl start`) over a period covering at least one hourly fetch and, if feasible, the weekly trigger
  - Confirm daily-note update cadence matches pre-migration behavior (refreshed on every successful fetch)
  - This task is a hard gate: do not proceed to task 11 without explicit confirmation that verification passed
  - _Requirements: 5.1_

- [ ] 11. Delete the custom scheduler [deletion]
  - Delete `run_native_schedules`, `run_agentic_schedules`, `NativeEntry`, `REGISTRY`, and the tick loop in `spawn` from `crates/backend/src/scheduler.rs`
  - Remove the now-dead `scheduler::spawn(state.clone())` line (and its module import if `scheduler.rs` becomes empty — decide whether the file/module is deleted entirely)
  - Decide and implement Requirement 5.2: either remove `interval_minutes`/`depends_on`/`enabled` from `NativeScheduleEntry`/`AgentEntry` in `crates/common/src/config.rs`, or keep them and document them as informational-only in `KB_README.md`/`config.toml` comments
  - Verify: `cargo build --workspace` and `cargo clippy --workspace` report no new errors or warnings; grep confirms no remaining callers of the deleted functions
  - _Requirements: 5.1, 5.3, 5.4_

- [ ] 12. Record the two deferred duplications
  - Add a short note (README or design-doc cross-reference) naming `generate_daily()` vs. `daily-writer` and `route_new_tasks_semantic` vs. `route_new_tasks_sync` as known, deliberately-unresolved duplications, pointing at this spec's Resolved Questions section
  - _Requirements: 6.1, 6.2, 6.3_
