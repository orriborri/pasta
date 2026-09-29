# Design Document

## Context

Phase 2 finishes the repurpose: pasta becomes a pure read-only engine and the
last deterministic vault writers move to KiroCrew. This is the architecturally
cleanest end state — one write authority, a side-effect-free MCP contract — but
it is also where the real risk lives, because it trades pasta's most valuable
code (deterministic, idempotent, rule-based `create_task`/`classify`) for an LLM
host that, at the time Phase 1 was written, was crash-looping.

This spec is therefore **gated**. Its design is recorded now so the decision and
the parity bar are captured, but its deletion tasks must not run until the gate
in Requirement 1 holds. `tasks.md` encodes that: the deletion phases are blocked
behind explicit precondition tasks and must not be checked off speculatively.

## Goals / Non-Goals

**Goals:**
- Reach a pasta that performs zero vault mutation
- Move `generate_daily`, task materialization, and (if still wanted)
  `repair_links`/`audit_para` to KiroCrew with proven parity
- Preserve the deterministic guarantees of `create_task`/`classify` by
  reimplementing them as zero-token crons, not LLM subagents

**Non-Goals:**
- Executing before the gate holds (gateway stable + parity proven)
- Re-deriving classification as LLM judgment — the declarative rules must
  survive the move
- Any change to the engine (fetchers/pipeline/storage/search/MCP read-API)
- Consolidating the four storage engines (separate follow-up from
  `remove-accreted-architecture` task 18)

## Decisions

**Parity-first, delete-last.** For each writer, stand up and verify the KiroCrew
replacement against real vault inputs *before* removing the Rust. Never delete a
writer and build its replacement in the same step. Alternative considered:
delete-then-rebuild for a clean diff — rejected, it guarantees a window with no
writer on the user's live vault.

**Task creation moves as a zero-token cron, not an LLM.** `create_task` is pure
mechanism: deterministic filename, early-return-if-exists, declarative
classification. Reimplementing it as a KiroCrew `script=` cron preserves
idempotency and auditability at zero token cost; an LLM subagent would
reintroduce duplication and misroute risk. Alternative considered: LLM subagent
for "smart" routing — rejected, it degrades a testable rule table into
probabilistic judgment.

**`generate_daily` moves as a deterministic materializer + optional LLM
enrichment.** The `## 🔗 Activity Feed` splice is mechanical and belongs in a
zero-token cron; any prose/summary enrichment on top is where a KiroCrew
subagent adds value. Split the two so the mechanical part keeps its
in-place-splice guarantees. Alternative considered: one LLM writes the whole
daily note — rejected, it loses the deterministic feed and risks clobbering
user-authored sections.

**Decide `repair_links`/`audit_para` explicitly, don't drop them silently.**
Either they get a named KiroCrew home or they are consciously retired. A silent
loss during deletion is the failure mode this decision exists to prevent.

**Gateway stability is a precondition, owned outside this spec.** Phase 2 does
not fix KiroCrew; it refuses to start until KiroCrew is fixed. That fix is
tracked separately (see the `project.kirocrew.gateway_stability` note).

## Risks

- **Parity is asserted, not proven.** Mitigation: Requirement 1.2 demands
  demonstrated output parity on real inputs; the per-writer tasks below each
  carry a "verify parity" step before their deletion step.
- **KiroCrew instability recurs mid-cutover.** Mitigation: the gate is not a
  one-time check — if the gateway destabilizes, deletion pauses and pasta's
  writers remain authoritative (they are only removed at the very end).
- **Split-brain window (two writers).** Mitigation: run the KiroCrew replacement
  in shadow (writing to a scratch path or comparing output) until parity is
  signed off, then cut over and delete in one commit per writer.
