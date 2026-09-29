# Requirements Document

## Introduction

`pasta-backend` currently owns three unrelated responsibilities inside one binary and one `tokio::spawn` loop: the IPC socket server for TUI/MCP clients, a hand-rolled scheduler (`scheduler.rs`'s tick loop plus `NativeScheduleEntry`/`AgentEntry` interval and `depends_on` tracking), and vault-GTD domain logic (`vault_organize.rs`, `fetchers/vault_manager.rs`) that the scheduler calls directly. There is no OS-level supervision of any of it: the daemon is started by hand via `start.sh`, and it has been silently dead since 2026-07-31 with nothing to notice or restart it. In the same period, a parallel bash cron script (`gtd-hourly.sh`) attempting to do the same job independently failed on every invocation since 2026-06-02 and was never fixed, because nothing monitored it either.

Two concrete defects follow directly from this coupling:

- `fetch_cycle::run` — whose own doc comment says it "fetches via kb-fetchers, writes feeds, indexes... and generates the daily note" — calls `vault_organize::generate_daily()` inline, so a function about ingesting external data is also responsible for vault-note generation. It cannot be scheduled, tested, or reasoned about independently of that call.
- `vault_organize::generate_weekly()` exists and is exposed as the `--weekly` CLI flag, but nothing schedules it. It has been dead code from a scheduling standpoint since it was written.

This spec replaces the custom scheduler with systemd user timers and unit dependencies, and gives every unit of scheduled work a standalone, exit-code-bearing CLI entrypoint so systemd (not application code) owns "when does this run" and "what does this depend on." It does not change what any job does — only what triggers it and how failures are surfaced.

## Non-Negotiable Sequencing Constraint

`.ralph/state.json` shows the `remove-accreted-architecture` spec is mid-execution (currently task 9, "Resolve the kb data directory once") and touches `scheduler.rs`, `config.rs`, `fetch_cycle.rs`, and `commands.rs` — the same files this spec must change. **This spec's tasks MUST NOT be queued to Ralph, and no manual edits under this spec MUST land, until `remove-accreted-architecture` reaches its final task (19) and its own acceptance criteria are met.** Running both concurrently, even in separate worktrees, would produce conflicting rewrites of the same functions (`Command::ForceFetch`'s new guard, the merged completed-fetcher set, `fetch_cycle::run`'s signature).

## Glossary

- **Native schedule**: a `[schedules.<name>]` entry in `~/.pasta/config.toml` with `interval_minutes`/`enabled`, currently polled by `scheduler.rs`'s `run_native_schedules`
- **Agentic schedule**: a `[[agents]]` entry in `~/.pasta/config.toml` with `name`/`prompt`/`cwd`/`depends_on`, currently polled by `run_agentic_schedules` and executed via `process::spawn_agent`
- **Oneshot mode**: `pasta-backend` invoked with a flag (`--fetch`, `--daily`, `--weekly`, `--organize`, `--route-tasks`, `--sync-trello`) that runs one job and exits, as opposed to **daemon mode** (no flags), which binds the IPC socket and runs forever
- **Fetch cycle**: `fetch_cycle::run`, which fetches gmail/linear/calendar/slack via `kb_sync::fetch`, writes `.feeds/`, indexes into search stores, and currently also calls `generate_daily()`
- **Vault-GTD domain**: the personal-productivity logic in `vault_organize.rs` and `fetchers/vault_manager.rs` — PARA audit, link repair, daily/weekly note generation, task archiving/routing — as distinct from the KB ingestion/search logic in the `kb-*` crates
- **Unit chaining**: using systemd's `OnSuccess=` (or `Wants=`/`After=` on the triggered unit) so a service starts only after another service unit exits successfully, replacing `depends_on`
- **Lingering**: `loginctl enable-linger <user>`, required for a user's systemd units (including timers) to run when the user is not logged in

## Requirements

### Requirement 1

**User Story:** As the maintainer, I want every schedulable unit of work exposed as a standalone command with a meaningful exit code, so that systemd — not application code — can decide when it runs and what it depends on.

#### Acceptance Criteria

1. WHEN `pasta-backend` is invoked with `--fetch` THEN it SHALL run only the fetch/write-feeds/index steps of the current fetch cycle and SHALL NOT call `generate_daily()`
2. WHEN `pasta-backend` is invoked with `--daily` THEN it SHALL call `generate_daily()` exactly as `cli::daily()` does today, unchanged in behavior
3. WHEN `pasta-backend` is invoked with `--weekly` THEN it SHALL call `generate_weekly()` exactly as `cli::weekly()` does today, unchanged in behavior
4. WHEN `pasta-backend` is invoked with a new `--vault-maintenance` flag THEN it SHALL call `fetchers::vault_manager::run()`, which currently has no CLI entrypoint and is only reachable through the internal scheduler
5. WHEN `pasta-backend` is invoked with a new `--run-agent <name>` flag THEN it SHALL resolve `<name>` against the `[[agents]]` entries in config, spawn it via the same mechanism `process::spawn_agent` uses today (same log file naming, same timeout), block until it finishes, and exit non-zero if the agent process failed or timed out
6. WHEN any oneshot flag's underlying operation fails THEN the process SHALL exit with a non-zero status, so a systemd `OnSuccess=` chain does not fire on failure
7. WHEN any oneshot flag's underlying operation succeeds THEN the process SHALL exit 0

### Requirement 2

**User Story:** As the maintainer, I want daemon mode to do nothing but serve the IPC socket, so that the scheduling responsibility has exactly one owner.

#### Acceptance Criteria

1. WHEN `pasta-backend` is run in daemon mode (no flags) THEN it SHALL bind the IPC socket and run `server::run` as it does today
2. WHEN `pasta-backend` is run in daemon mode THEN it SHALL NOT call `scheduler::spawn`
3. WHEN daemon mode starts THEN it SHALL still take the `~/.pasta/pasta.lock` exclusive lock and refuse to start a second instance, unchanged from today

### Requirement 3

**User Story:** As the maintainer, I want systemd timers and unit dependencies to reproduce every schedule and chain currently expressed in `config.toml`, so that removing the custom scheduler loses no coverage.

#### Acceptance Criteria

1. WHEN the migration is complete THEN a `pasta-fetch.timer`/`.service` pair SHALL run `pasta-backend --fetch` on the interval currently configured under `[schedules.gitlab/linear/slack/gmail/calendar]` (60 minutes)
2. WHEN `pasta-fetch.service` exits 0 THEN `pasta-daily.service` (running `--daily`) SHALL start, reproducing the current every-fetch-cycle daily note refresh
3. WHEN `pasta-fetch.service` exits 0 THEN `pasta-inbox-processor.service` (running `--run-agent inbox-processor`) SHALL start, reproducing the current `depends_on = ["gitlab-fetcher","gmail-fetcher","slack-fetcher","linear-fetcher"]` chain
4. WHEN `pasta-inbox-processor.service` exits 0 THEN `pasta-daily-writer.service` (running `--run-agent daily-writer`) SHALL start, reproducing the current `depends_on = ["inbox-processor"]` chain
5. WHEN the migration is complete THEN a `pasta-vault-maintenance.timer`/`.service` pair SHALL run `pasta-backend --vault-maintenance` on the interval currently configured under `[schedules.vault-maintenance]` (1440 minutes)
6. WHEN the migration is complete THEN a `pasta-weekly.timer`/`.service` pair SHALL run `pasta-backend --weekly` on a weekly cadence, closing the gap where `generate_weekly()` has never been scheduled
7. WHEN the migration is complete THEN a `pasta-trello.timer`/`.service` pair SHALL run `pasta-backend --sync-trello` on the interval currently configured under `[schedules.trello]`, gated the same way `cli::sync_trello()` gates on `[trello] enabled`
8. WHEN any of the above timers or chained services fail THEN the failure SHALL be visible via `systemctl --user status`/`journalctl --user -u <unit>` without needing to read `pasta/logs/`

### Requirement 4

**User Story:** As the maintainer, I want the daemon and its scheduled units to survive reboots and restart after crashes, so that "silently dead since a date nobody noticed" cannot recur.

#### Acceptance Criteria

1. WHEN the machine boots THEN `pasta-backend.service` (daemon mode) SHALL start automatically, without running `start.sh` by hand
2. WHEN `pasta-backend.service` exits unexpectedly THEN systemd SHALL restart it (`Restart=on-failure`)
3. WHEN the machine is off or suspended past a `pasta-fetch.timer` firing time THEN the timer SHALL run at the next opportunity after boot/resume (`Persistent=true`), not silently skip the missed run
4. WHEN the user is not logged in via an interactive session THEN the user's systemd units SHALL still run, which requires lingering to be enabled and verified (`loginctl show-user <user> --property=Linger`)
5. WHEN `start.sh`'s current pkill-and-relaunch dance is replaced THEN restarting the daemon after a rebuild SHALL be `systemctl --user restart pasta-backend.service`, and `start.sh`/`run-sync.sh` SHALL be updated or removed accordingly

### Requirement 5

**User Story:** As the maintainer, I want the old scheduler deleted once the systemd units are proven equivalent, so that there is exactly one scheduling mechanism, not two coexisting ones.

#### Acceptance Criteria

1. WHEN the systemd units have run successfully through at least one full cycle of every job (fetch, daily, weekly, vault-maintenance, both agents, trello) THEN `scheduler.rs`'s `run_native_schedules`, `run_agentic_schedules`, `NativeEntry`, `REGISTRY`, and the `spawn` tick loop SHALL be deleted
2. WHEN the scheduler is deleted THEN `[schedules.*]` and `[[agents]]` interval/`depends_on` fields in `~/.pasta/config.toml` SHALL either be removed from the config struct or documented as informational-only (superseded by the unit files), with a decision recorded in the design doc
3. WHEN deletion happens THEN it SHALL be its own commit, separate from the commits that introduce the unit files, so a regression is attributable to one or the other
4. WHEN the workspace is built after deletion THEN `cargo build --workspace` and `cargo clippy --workspace` SHALL report no new errors or warnings

### Requirement 6

**User Story:** As the maintainer, I want the two known behavioral duplications this spec surfaces recorded rather than silently resolved, so that a product decision isn't made as a side effect of an infra migration.

#### Acceptance Criteria

1. WHEN this spec is implemented THEN it SHALL leave `generate_daily()` (mechanical, native) and the `daily-writer` agent (LLM-authored) both running, in their current order, because deciding whether they should be merged or one should be dropped is a content decision, not a scheduling one
2. WHEN this spec is implemented THEN it SHALL leave `vault_manager::route_new_tasks_semantic` (used by `--route-tasks` and the agent path) and `vault_manager::route_new_tasks_sync` (used inside native `vault-maintenance`) both in place, unmerged, because reconciling two task-routing implementations is out of scope here
3. WHEN either duplication is left in place THEN it SHALL be named explicitly in the design doc's Non-Goals, not merely omitted
