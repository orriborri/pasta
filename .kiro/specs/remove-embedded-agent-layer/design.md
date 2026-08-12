# Design Document

> **Reconciliation (2026-08-12):** the "keep `acp.rs`/TUI chat, delete by mechanism not by file" decision below is obsolete — `remove-accreted-architecture` already deleted the TUI and the orphaned ACP chat surface (commit `42f8fd1`); this spec now removes only the scheduled agentic layer + `generate_weekly`.

## Context

The repurpose target is: pasta becomes a headless ingestion + hybrid-search
engine exposing an MCP read-API, and KiroCrew (cron jobs + subagents) owns the
agent layer that reasons over and writes the vault. Two independent analyses
(a Shape A "pure read-only engine" steelman and a Shape B "keep deterministic
writers" steelman) converged on a phased **B→A** path: delete the fragile
agentic layer now, keep the deterministic Rust writers until KiroCrew parity is
proven, then converge on pure read-only. This spec is Phase 1 (the B step).

Investigation established the decisive boundary fact: the scheduled agents and
the interactive TUI chat use **different** mechanisms.

- **Scheduled agentic layer** — `scheduler.rs::run_agentic_schedules` iterates
  the `[[agents]]` config and calls `process.rs::spawn_agent`, which forks
  `kiro-cli chat --no-interactive --trust-all-tools --agent <name> <prompt>`
  with a kill-on-timeout, tracked in `state.process` (`running`/`completed`,
  `depends_on` chaining, `AgentStarted`/`AgentFinished` events). The two
  configured agents (`inbox-processor`, `daily-writer`) duplicate work the
  deterministic writers already do.
- **ACP TUI chat** — `commands.rs` handles `Command::ChatStart` via
  `acp::spawn`, `ChatPrompt` via `acp::send_prompt`, `ChatStop` via the handle
  in `conn.acp`. This is `acp.rs`'s only caller. It is **not** reached by the
  scheduler and does not use `spawn_agent`.

Because the two are disjoint, Phase 1 deletes the scheduled layer and leaves
`acp.rs` and the TUI chat entirely untouched — a smaller, safer cut than the
"delete acp.rs" assumption both steelmen started from.

The write race this closes: today the same daily/weekly files are written by
(1) deterministic Rust, (2) pasta-spawned `kiro-cli` agents, and (3) KiroCrew
crons. Phase 1 removes writer (2) and retires the weekly half of (1) in favour
of (3)'s superior pipeline, leaving deterministic Rust + KiroCrew — a boundary
Phase 2 then collapses to one.

## Goals / Non-Goals

**Goals:**
- Remove the scheduled agentic layer (`spawn_agent`, `run_agentic_schedules`,
  `[[agents]]`, its process-tracking and events) so the daemon never forks an
  LLM CLI on a timer
- Retire `generate_weekly` and its CLI in favour of the KiroCrew weekly pipeline
- Keep `acp.rs`/TUI chat and every deterministic writer behaviourally unchanged
- Land clean under the workspace build/clippy/test gate

**Non-Goals:**
- Deleting `vault_organize.rs`/`inbox.rs` deterministic writers. That is
  Phase 2 (`headless-engine-cutover`), gated on KiroCrew parity — see that spec.
- Touching `acp.rs` or the interactive chat in any way.
- Moving daily-note or task-creation logic to KiroCrew. Phase 1 only *removes
  the scheduled LLM duplicate*; the deterministic path stays authoritative.
- Any change to fetchers, pipeline, storage, hybrid search, or the MCP read-API.
- Fixing KiroCrew gateway stability. Real and a prerequisite for Phase 2, but
  out of scope here.

## Decisions

**Delete the scheduled layer, keep `acp.rs`.** The scheduled path
(`spawn_agent` + `run_agentic_schedules`) is the flaky, low-value duplicate;
the ACP chat is a distinct, wanted feature with no scheduler coupling. Deleting
by *mechanism* (scheduled fork) rather than by *file* (`acp.rs`) preserves the
chat for free. Alternative considered: delete `acp.rs` too — rejected, it costs
the TUI chat for no Phase-1 benefit, since the chat is not part of the write
race.

**Retire `generate_weekly` now, keep `generate_daily` for now.** The weekly
summary has a proven, better replacement already running in KiroCrew (vault →
`status:ready` gate → Google Doc via `gog`, crons `d0b28e3a`/`9b0585a9`), so
removing it is a clean win with no capability loss. The daily writer's
deterministic `## 🔗 Activity Feed` splice has *no* proven KiroCrew equivalent
yet, so it stays until Phase 2. Alternative considered: retire both weekly and
daily now — rejected, it would leave the daily note unwritten until KiroCrew
parity exists, i.e. it smuggles Phase 2's risk into Phase 1.

**Remove the `[[agents]]` config surface entirely, not just its two entries.**
The `agents` Vec, `Agent` struct, `depends_on` chaining, and
`agent_timeout_minutes` exist only to serve the scheduled layer. Leaving an
empty, unread config surface is exactly the accreted dead code the sibling
`remove-accreted-architecture` change exists to eliminate. Alternative
considered: keep the config, drop the entries — rejected as dead surface.

**Sequence after `remove-accreted-architecture`.** That in-flight change edits
`vault_organize.rs`, `inbox.rs`, `scheduler.rs`, and introduces `VaultLayout`;
the working tree is currently dirty mid-refactor. Phase 1 touches the same
files, so it lands *after* that change is committed and green, to avoid
conflicting edits and an ambiguous base. This is a hard prerequisite, not a
preference.

## Risks

- **A scheduled agent is doing something the deterministic writer does not.**
  Mitigation: the two configured prompts are literally "Process inbox and feeds
  into the vault." and "Create or update today's daily note." — both covered by
  `inbox::process` and `generate_daily`. If review finds a divergence, capture
  it as a Phase 2 parity requirement rather than blocking Phase 1.
- **Hidden `spawn_agent` caller.** Mitigation: task 1 greps all call sites
  (`run_agentic_schedules` and the manual command in `commands.rs`) before
  deleting.
- **Config parse break for the live daemon.** Mitigation: task 4 verifies the
  daemon loads `pasta.toml` with `[[agents]]` removed.
