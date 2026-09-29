# Design Document

## Context

`pasta-backend` already has more of the seam this spec needs than it first appears. `main.rs` dispatches on flags before ever entering daemon mode: `--organize`, `--daily`, `--weekly`, `--fetch`, `--route-tasks`, `--sync-trello` each run one job and exit. `--daily` and `--weekly` already call `generate_daily()`/`generate_weekly()` directly and need no code change at all — `generate_weekly()` simply has no systemd timer pointed at it yet, which is the whole gap.

What's missing is narrower than "build a CLI": `--fetch` still calls the full `fetch_cycle::run`, which internally calls `generate_daily()` — so even in oneshot mode, fetching and daily-note generation aren't independent. `fetchers::vault_manager::run()` (archive-done, archive-stale, sync task routing — what the native `vault-maintenance` schedule actually calls) has no CLI flag at all; only the *semantic* routing path (`vault_manager::route_new_tasks_semantic`, via `--route-tasks`) is exposed, which is a second, different routing implementation. And nothing exposes `process::spawn_agent` outside the scheduler's `run_agentic_schedules` loop — there is no way to run `inbox-processor` or `daily-writer` from the command line today.

Daemon mode (`main.rs`, no flags) binds a `UnixListener` and calls `server::run`, which loops forever accepting TUI/client connections — that part is a genuine long-running service and is untouched by this spec. It also calls `scheduler::spawn(state.clone())`, which starts the `tokio::spawn` loop this spec removes: a 6-second `interval` tick that polls `NativeScheduleEntry`/`AgentEntry` definitions from `~/.pasta/config.toml`, tracks `last_run`/`running_native` per name, and tracks agent completion in a `HashSet<String>` (`ps.completed`) consumed by `depends_on`.

None of this has been supervised by the OS. `start.sh` is a manual launcher: it builds if stale, `pkill`s any previous `pasta-backend`, and execs the new one in the foreground. `run-sync.sh` wraps the same build step in `systemd-run --user --scope`, which creates a *transient* unit for resource limits (`MemoryMax=28G`) — not a persistent, boot-surviving service. Neither script has run since 2026-07-31 (last `pasta/logs/` activity), and nothing alerted anyone. In the same window, an entirely separate bash cron job (`gtd-hourly.sh`, in the Obsidian vault's `.scripts/`, unrelated to this codebase) was independently trying to do part of the same job — hourly capture and a daily briefing — and has failed on every run since 2026-06-02. Two schedulers, both silently dead, is the recurring failure mode this spec exists to close off, on the pasta side specifically.

`Cargo.toml`'s workspace members do not list `crates/tui`, even though `crates/tui` exists on disk and the sibling `remove-accreted-architecture` spec's task 14 references it. This spec does not resolve that discrepancy; it is noted under Risks so it isn't mistaken for something this design already accounted for.

## Goals / Non-Goals

**Goals:**
- Give every schedulable job a standalone, exit-code-bearing CLI entrypoint on the `pasta-backend` binary
- Decouple fetch from daily-note generation, so each can be triggered and fail independently
- Close the `generate_weekly()` scheduling gap
- Reproduce every current `[schedules.*]` interval and `[[agents]]` `depends_on` chain as systemd timers and unit dependencies
- Make the daemon and every scheduled job survive reboot and restart on crash, without a human running a shell script
- Delete the custom scheduler once the systemd units are proven equivalent, leaving exactly one scheduling mechanism

**Non-Goals:**
- Merging `generate_daily()` (native) and the `daily-writer` agent (LLM), or deciding one should be dropped — a content decision, recorded as a Resolved Question below, not decided here
- Merging `vault_manager::route_new_tasks_semantic` and `route_new_tasks_sync` into one routing implementation — recorded, not fixed here
- Anything `remove-accreted-architecture` already owns: the `ForceFetch` concurrency guard, `crates/mcp`/`crates/search` deletion, `VaultLayout`, the kb subprocess boundary, the recursive task-set fix. This spec assumes that work has landed and does not re-touch those call sites beyond what Requirement 3 needs.
- Resolving the `crates/tui` / `Cargo.toml` workspace-members discrepancy noted in Context
- Changing what any agent prompt says or what `vault_organize`/`vault_manager` functions do internally — only what triggers them and how failure is surfaced
- Introducing a workflow engine (Airflow/Temporal/n8n-class tooling). Rejected outright: this is ~7 jobs on a single-user machine; systemd, already relied on for `crond`/timers elsewhere on this box, provides interval scheduling, dependency chaining (`OnSuccess=`), retries, logging (`journalctl`), and boot-persistence (`Persistent=true`) for zero new dependencies. A heavier engine would trade a hand-rolled scheduler for an operated one, which is a worse trade for this scale.

## Decisions

**Split `fetch_cycle::run` at the `generate_daily()` call.** Remove the call from `fetch_cycle::run` entirely; `--fetch` becomes fetch+write-feeds+index only. `pasta-daily.service` is triggered by `OnSuccess=` from `pasta-fetch.service`, so the observed cadence (daily note refreshed every successful fetch cycle) is unchanged — only the mechanism moves from an inline function call to a unit dependency. Alternative considered: keep the call and just also add a weekly-style timer for daily notes as a backstop — rejected, it would run `generate_daily()` twice on the normal path for no reason.

**Reuse existing CLI flags as the command each systemd service unit execs**, adding only the two that don't exist: `--vault-maintenance` (wraps `fetchers::vault_manager::run()`, mirroring how `--route-tasks` already wraps `vault_manager::route_new_tasks_semantic()`) and `--run-agent <name>` (looks up the named `[[agents]]` entry, calls the same `process::spawn_agent` path used today, blocks on the returned `JoinHandle`, and maps timeout/spawn failure to a non-zero exit). This keeps `process::spawn_agent`'s existing behavior — same `pasta/logs/<agent>_<timestamp>.log` naming, same `agent_timeout_minutes` config — while making it reachable outside the scheduler loop. Alternative considered: a wholly separate small binary/crate for agent-running — rejected, `spawn_agent` already lives in `backend` and depends on `backend`'s config/log helpers; a new crate would just be an extra `Cargo.toml` and PATH entry for no isolation benefit.

**Daemon mode drops the `scheduler::spawn(state.clone())` call, keeps everything else.** The IPC server, the lock file, the memory-log ticker, and graceful shutdown on `ctrl_c` are unrelated to scheduling and stay exactly as they are. This is a one-line removal in `main.rs`, deferred until Requirement 5's units are verified (see Migration Plan) — the call is commented as superseded, not deleted, until then, so a revert is trivial if a unit misbehaves.

**One systemd user service for the daemon, timers for everything else.**
- `pasta-backend.service` — `Type=simple`, runs `pasta-backend` (daemon mode, no flags), `Restart=on-failure`, `WantedBy=default.target`. Replaces `start.sh`'s pkill-and-relaunch and `run-sync.sh`'s transient `systemd-run --scope`. The `MemoryMax=28G`/`MemoryHigh=20G` limits `run-sync.sh` passes to `systemd-run` move into the unit file's `[Service]` section (`MemoryMax=`, `MemoryHigh=`) so they persist across restarts instead of depending on which launcher script was used.
- `pasta-fetch.timer` (`OnCalendar=hourly`, `Persistent=true`) → `pasta-fetch.service` (`Type=oneshot`, execs `pasta-backend --fetch`).
- `pasta-daily.service` (`Type=oneshot`, execs `--daily`) — `OnSuccess=` triggered by `pasta-fetch.service`. No timer unit of its own.
- `pasta-inbox-processor.service` (`--run-agent inbox-processor`) — `OnSuccess=` triggered by `pasta-fetch.service`.
- `pasta-daily-writer.service` (`--run-agent daily-writer`) — `OnSuccess=` triggered by `pasta-inbox-processor.service`.
- `pasta-vault-maintenance.timer` (`OnCalendar=daily`, `Persistent=true`) → `pasta-vault-maintenance.service` (`--vault-maintenance`).
- `pasta-weekly.timer` (`OnCalendar=Fri 15:00`, `Persistent=true`) → `pasta-weekly.service` (`--weekly`). Friday 15:00 matches the day/hour gate `gtd-hourly.sh` used for its (never-working) weekly review, kept here only as a reasonable default — open to changing.
- `pasta-trello.timer`/`.service` — interval and enable-gate taken from `[schedules.trello]`/`[trello] enabled`, execs `--sync-trello`.

All timers and services are **user** units (`systemctl --user`), matching where `crond` already runs for this user, and require `loginctl enable-linger $(whoami)` — checked explicitly as an acceptance criterion, because the machine-off/late-crond-start failure mode that started this whole investigation is exactly what lingering + `Persistent=true` is meant to close.

**Delete the scheduler only after a full cycle proves the units equivalent, as its own commit.** `run_native_schedules`, `run_agentic_schedules`, `NativeEntry`/`REGISTRY`, and the tick loop in `scheduler.rs` are deleted in a dedicated final task, not alongside the units that replace them — matching `remove-accreted-architecture`'s own "sequence deletions before/after behaviour changes, one phase per commit" approach, so a regression is attributable to one change or the other.

## Risks / Trade-offs

[`crates/tui` isn't in `Cargo.toml`'s workspace members but exists on disk and is referenced by the sibling spec] → Not this spec's problem to resolve; noted so it isn't mistaken for an oversight in this design. If `cargo build --workspace` doesn't touch it, the TUI's build status is orthogonal to this migration.

[Systemd `OnSuccess=` chains are coarser than the old per-fetcher `depends_on` tracking — `pasta-fetch.service` is one unit, not five] → No loss in practice: `fetch_cycle::run` already treats the fetch group as one atomic pass today (its own comment notes gitlab arrives via the gmail feed and never produces its own records, so the five-name completion set was already fictional granularity over one real operation).

[Decoupling `generate_daily()` from `--fetch` and re-attaching it via `OnSuccess=` changes the failure mode: a daily-note bug can no longer make the fetch itself look failed, but a fetch failure will now correctly prevent the daily note from refreshing with stale data] → This is the intended behavior change, not a side effect; called out so it isn't mistaken for a regression during verification.

[Config sprawl during the transition: `~/.pasta/config.toml`'s `[schedules.*]` intervals and `[[agents]]` `depends_on` become dead configuration once units exist, but Requirement 5 defers deleting them from the config struct until after a full verified cycle] → There is a window where the config file's timing values and the systemd units' timing values can drift and only the units are authoritative. Documented in the migration plan so whoever runs it knows to update the timer's `OnCalendar=` rather than the config file if intervals change during that window.

[Lingering must be enabled for user timers to fire when not logged in, and this is a one-time host-level `loginctl` command outside the repo] → Made an explicit acceptance criterion (Requirement 4.4) rather than an assumption, precisely because an unnoticed missing prerequisite is the class of bug this whole spec responds to.

[`--run-agent` blocking synchronously on an async `JoinHandle` from a CLI `main` that isn't already inside the daemon's tokio runtime] → `pasta-backend` is already a `#[tokio::main]` binary, so oneshot flags run inside a tokio runtime today (`--fetch` already `.await`s `fetch_cycle::run`); `--run-agent` follows the same pattern and needs no new runtime setup.

[Ralph is mid-flight on `remove-accreted-architecture`, touching the same files] → Addressed as a hard sequencing constraint in requirements.md, not merely a note here.

## Migration Plan

1. Wait for `remove-accreted-architecture` to complete (all 19 tasks, `cargo build`/`clippy` clean). Do not start phase 2 before this.
2. Add `--vault-maintenance` and `--run-agent <name>` flags to `main.rs`; extract `generate_daily()`'s call out of `fetch_cycle::run` into the `--daily` path only. Verify each flag manually: `pasta-backend --fetch`, `--daily`, `--weekly`, `--vault-maintenance`, `--run-agent inbox-processor`, `--run-agent daily-writer` all exit 0 on success and non-zero on induced failure (e.g. rename the vault path temporarily).
3. Write the unit files (`pasta-backend.service`, the five timer/service pairs, the two `OnSuccess=`-chained services) under `~/.config/systemd/user/`. Enable lingering. `systemctl --user daemon-reload`, enable and start `pasta-backend.service` and the timers.
4. Let one full cycle run for every job (fetch → daily + inbox-processor → daily-writer; vault-maintenance; weekly; trello). Confirm via `journalctl --user -u <unit>` and `pasta/logs/` that each ran, and that daily-note cadence matches the pre-migration behavior.
5. Comment out (not delete) `scheduler::spawn(state.clone())` in daemon mode. Confirm the daemon still serves the IPC socket with no scheduler running.
6. Update `start.sh`/`run-sync.sh` to `systemctl --user restart pasta-backend.service` (or remove them if the unit file fully supersedes their build-and-relaunch role — decide based on whether the build step still needs to live somewhere).
7. Once the above has run cleanly for at least a few days, delete `scheduler.rs`'s dead code (`run_native_schedules`, `run_agentic_schedules`, `NativeEntry`, `REGISTRY`, the tick loop) and the now-superseded `scheduler::spawn` line, as one commit. Decide and record whether `[schedules.*]`/`[[agents]]` config fields are removed from the struct or kept as informational (Requirement 5.2).

Rollback is per-phase `git revert` for the code changes; the systemd units are additive and can be `systemctl --user disable --now`'d independently without touching code, which is why the scheduler deletion is deliberately the last step, not the first.

## Resolved Questions

- **`generate_daily()` and the `daily-writer` agent both stay, unmerged.** One is a mechanical skeleton refresh, the other is LLM-authored narrative content layered on top via `OnSuccess=` chaining after `inbox-processor`. Whether they should be merged is a product decision for the vault-gtd domain, out of scope here (Requirement 6.1).
- **`route_new_tasks_semantic` and `route_new_tasks_sync` both stay, unmerged.** Native `vault-maintenance` keeps using the sync/keyword fallback it uses today; `--route-tasks` keeps using the semantic path. Reconciling them is out of scope here (Requirement 6.2).
- **Weekly cadence defaults to Friday 15:00**, matching `gtd-hourly.sh`'s (nonfunctional) intent, purely as a starting point — trivially changed later via the timer's `OnCalendar=` since that's now the single source of truth for timing.

## Execution Notes for Ralph

**Do not queue this spec until `remove-accreted-architecture`'s `work_queue` is empty and `.ralph/state.json` shows it complete.** Both specs touch `scheduler.rs`, `fetch_cycle.rs`, `commands.rs`, and `config.rs`; running them in parallel worktrees would merge-conflict on nearly every task.

**Run with `execution.parallel: 1` for this spec**, for the same reason `remove-accreted-architecture` does: task 2 (CLI flags) and the later scheduler-deletion task both touch `main.rs` and `scheduler.rs` repeatedly.

**Tasks that only add systemd unit files have no meaningful RED phase** — there's no Rust test for "does this `.timer` file exist with the right `OnCalendar=`." Their verification contract is: `systemctl --user daemon-reload` succeeds, `systemctl --user list-timers` shows the expected next-run time, and a manually triggered `systemctl --user start <service>` exits 0 and produces the expected side effect (new daily note content, new `pasta/logs/` entry, etc.). Tasks marked `[unit-file]` in tasks.md use that contract instead of a Rust test.

**Task ordering is load-bearing.** The scheduler-deletion task (final task) must not run until every unit-file task and the manual full-cycle verification step have completed — it is gated on human confirmation, not on another task's file changes, so Ralph should treat it as blocked until that confirmation is given explicitly.
