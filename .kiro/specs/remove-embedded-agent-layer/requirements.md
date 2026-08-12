# Requirements Document

> **Reconciliation (2026-08-12):** the ACP TUI chat this spec assumed it would preserve was already deleted by `remove-accreted-architecture` (TUI crate + `acp.rs` + `ChatStart`/`ChatPrompt`/`ChatStop` + `Event::Chat`, commit `42f8fd1`), so Requirement 2 (keep the TUI chat working) is VOID; Requirements 1/3/4/5 stand.

## Introduction

pasta writes the user's Obsidian vault through two unrelated mechanisms. The
interactive TUI chat drives `kiro-cli` over ACP (`crates/backend/src/acp.rs`,
reached only via `Command::ChatStart`/`ChatPrompt`/`ChatStop`). Separately, a
scheduled *agentic* layer forks one-shot `kiro-cli chat --no-interactive
--trust-all-tools` processes — `crates/backend/src/process.rs::spawn_agent`,
driven by `crates/backend/src/scheduler.rs::run_agentic_schedules` from the
`[[agents]]` config (`inbox-processor`, `daily-writer`). These scheduled agents
do the same jobs as pasta's deterministic Rust writers (`inbox.rs::process`,
`vault_organize.rs::generate_daily`) and as KiroCrew crons that already write
the vault — three writers racing on the same files.

This spec (Phase 1 of the headless-engine repurpose) removes the *scheduled
agentic layer* and retires `generate_weekly`, whose job KiroCrew already does
better. It deletes no deterministic writer and does not touch the ACP TUI chat.
It is the low-risk cut that collapses the write race from three mechanisms to
two, ahead of the gated Phase 2 (`headless-engine-cutover`) that moves the
remaining deterministic writers to KiroCrew.

## Glossary

- **Scheduled agentic layer**: `spawn_agent`, `run_agentic_schedules`, the
  `[[agents]]` config, and the process-tracking that supports them — the path
  that forks `kiro-cli` on a timer to write the vault
- **ACP TUI chat**: the interactive chat in `acp.rs` reached through
  `Command::ChatStart`/`ChatPrompt`/`ChatStop`; unrelated to the scheduled layer
- **Deterministic writer**: pasta's in-process Rust vault mutation
  (`inbox.rs::process`→`create_task`, `vault_organize::generate_daily`,
  `repair_links`, `audit_para`) — reproducible, idempotent, unit-testable
- **Agent host**: KiroCrew (cron jobs + subagents) that reads pasta's MCP tools
  and owns LLM reasoning/enrichment over the vault

## Requirements

### Requirement 1

**User Story:** As the maintainer, I want the daemon to stop forking `kiro-cli`
on a schedule, so that a single unstable LLM subprocess can no longer wedge or
race the vault, and the scheduler has one responsibility.

#### Acceptance Criteria

1. WHEN `crates/backend` is compiled THEN `process.rs::spawn_agent` and any
   `AgentStarted`/`AgentFinished` events that only its call sites emit SHALL NOT
   exist
2. WHEN `scheduler.rs` is compiled THEN `run_agentic_schedules` SHALL NOT exist
   and `scheduler::spawn` SHALL invoke only `run_native_schedules`
3. WHEN `crates/common/src/config.rs` is compiled THEN the `agents` field, the
   `Agent` struct, and `agent_timeout_minutes` SHALL NOT exist
4. WHEN `/home/orre/.kiro/pasta.toml` is loaded THEN the daemon SHALL parse
   successfully with the `[[agents]]` blocks removed
5. WHEN a manual "run agent" command exists in `commands.rs` that calls
   `spawn_agent` THEN it AND its `ipc::Command` variant SHALL be removed
6. WHEN the process state (`state.rs`) is compiled THEN fields that exist only
   to track scheduled-agent subprocesses SHALL NOT exist

### Requirement 2

**User Story:** As a user of the pasta TUI, I want interactive chat to keep
working, so that removing the scheduled agents does not cost me a feature.

#### Acceptance Criteria

1. WHEN this change is complete THEN `crates/backend/src/acp.rs` SHALL still
   exist and compile
2. WHEN `Command::ChatStart`, `Command::ChatPrompt`, or `Command::ChatStop` is
   dispatched THEN it SHALL behave exactly as before this change
3. WHEN the codebase is searched THEN no ACP chat call site SHALL have been
   modified by this change

### Requirement 3

**User Story:** As a user, I want pasta's deterministic vault writers unchanged,
so that daily notes, task creation, and link repair keep working while the agent
layer moves out.

#### Acceptance Criteria

1. WHEN this change is complete THEN `inbox.rs::process`→`create_task`,
   `vault_organize::generate_daily`, `vault_organize::repair_links`, and
   `vault_organize::audit_para` SHALL exist and be unmodified in behaviour
2. WHEN `fetch_cycle.rs` runs THEN its post-fetch `generate_daily()` call SHALL
   remain
3. WHEN the `--daily`, `--organize`, and `--route-tasks` CLI commands are run
   THEN they SHALL behave exactly as before this change

### Requirement 4

**User Story:** As the maintainer, I want `generate_weekly` retired, so that
pasta stops maintaining a strictly inferior duplicate of KiroCrew's weekly
pipeline.

#### Acceptance Criteria

1. WHEN `vault_organize.rs` is compiled THEN `generate_weekly` SHALL NOT exist
2. WHEN the CLI is invoked THEN the `--weekly` command and its `commands.rs`/
   `cli.rs` wiring SHALL NOT exist
3. WHERE weekly-summary generation is removed THEN the owning KiroCrew weekly
   pipeline (vault → `status:ready` gate → Google Doc via `gog`, crons
   `d0b28e3a`/`9b0585a9`) SHALL be named in the design as its replacement
4. WHEN the change is reviewed THEN no other reference to
   `Meetings/Weekly/<date>-summary.md` generation SHALL remain in pasta

### Requirement 5

**User Story:** As the maintainer, I want the change to build clean and remove
nothing else, so that Phase 1 is a safe, reviewable slice.

#### Acceptance Criteria

1. WHEN the change is complete THEN `cargo build --workspace`,
   `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` SHALL
   pass with no new errors or warnings
2. WHEN the change is complete THEN no engine capability (fetchers, pipeline,
   storage, hybrid search, MCP read-API) SHALL be modified
3. WHEN the change is complete THEN no user-visible feature SHALL be removed
   other than scheduled agentic writes and weekly-summary generation
