# Requirements Document

## Introduction

This is Phase 2 of the headless-engine repurpose. Phase 1
(`remove-embedded-agent-layer`) removed pasta's scheduled `kiro-cli` agents and
retired `generate_weekly`, leaving two vault writers: pasta's deterministic Rust
(`inbox.rs::process`→`create_task`, `vault_organize::generate_daily`,
`repair_links`, `audit_para`) and KiroCrew crons. Phase 2 converges on a **pure
read-only engine**: pasta writes nothing to the vault, and the remaining
deterministic writers move to KiroCrew.

pasta's deterministic writers are its strongest asset — reproducible, idempotent
(`create_task` derives a stable `kb-<slug>.md` filename and early-returns if the
file exists), and rule-based (`task_match::classify` with an approval gate).
Replacing them with an LLM layer is the single biggest capability regression in
the whole repurpose. Therefore this phase is **gated**: it may only execute once
KiroCrew reproduces that behaviour with proven parity, and only after KiroCrew
gateway stability is fixed. Until then its deletion tasks stay blocked.

## Glossary

- **Read-only engine**: pasta ingests, indexes, and serves search/reads over
  MCP, and performs zero vault mutation
- **Parity**: a KiroCrew replacement produces the same vault output as the pasta
  writer it replaces, verified against real inputs, before the pasta writer is
  deleted
- **Zero-token cron**: a KiroCrew `script=`/`command=` cron that runs
  deterministic Python/shell with no LLM turn — the intended home for
  idempotent task creation, not an LLM subagent

## Requirements

### Requirement 1 (GATE — must hold before any deletion)

**User Story:** As the maintainer, I want hard preconditions verified before
pasta stops writing the vault, so that convergence never leaves the vault
unwritten or corrupted.

#### Acceptance Criteria

1. WHEN Phase 2 begins THEN the KiroCrew gateway crash-loop SHALL be resolved
   (no loopstall-watchdog bounces over a sustained observation window)
2. WHEN a deletion task is considered THEN a KiroCrew replacement for the writer
   it removes SHALL already exist and have demonstrated output parity
3. WHERE task creation is replaced THEN the replacement SHALL be a zero-token
   cron reproducing `create_task`'s deterministic filename and
   early-return-if-exists idempotency, NOT an LLM subagent
4. WHERE task classification is replaced THEN `task_match::classify`'s
   declarative rules and approval gate SHALL be reproduced with equivalent,
   auditable behaviour
5. IF any gate criterion is unmet THEN the deletion tasks SHALL remain blocked

### Requirement 2

**User Story:** As a user, I want daily-note activity content to keep appearing,
so that moving `generate_daily` to KiroCrew is invisible to me.

#### Acceptance Criteria

1. WHEN a KiroCrew replacement for `generate_daily` runs THEN it SHALL produce
   the `## 🔗 Activity Feed` section content equivalently, splicing in place and
   preserving surrounding note content
2. WHEN parity is confirmed on real data THEN `vault_organize::generate_daily`
   and its `fetch_cycle.rs` call SHALL be removed
3. WHEN `generate_daily` is removed THEN the `--daily` CLI command SHALL be
   removed

### Requirement 3

**User Story:** As a user, I want task materialization to keep working after it
moves, so that inbox signal still becomes task files with no duplicates.

#### Acceptance Criteria

1. WHEN the KiroCrew task-creation cron runs repeatedly THEN it SHALL NOT create
   duplicate task files for the same source record
2. WHEN parity is confirmed THEN `inbox.rs::process`/`create_task` and the
   `--route-tasks`/`--process-inbox` paths SHALL be removed from pasta
3. WHERE `repair_links` and `audit_para` are still wanted THEN each SHALL have a
   named KiroCrew home before being removed from pasta

### Requirement 4

**User Story:** As the maintainer, I want the end state to be a clean read-only
engine, so that the repurpose goal is literally satisfied.

#### Acceptance Criteria

1. WHEN Phase 2 is complete THEN `crates/backend/src/vault_organize.rs` SHALL
   NOT exist and `inbox.rs` SHALL retain no vault-write path
2. WHEN Phase 2 is complete THEN pasta SHALL contain no code that writes into
   the vault directory
3. WHEN Phase 2 is complete THEN the MCP read-API, fetchers, pipeline, storage,
   and hybrid search SHALL be unchanged
4. WHEN Phase 2 is complete THEN `cargo build/clippy/test --workspace` SHALL
   pass clean
