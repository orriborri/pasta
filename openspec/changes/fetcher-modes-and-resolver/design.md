## Context

Today each kb-fetcher is a bare struct with a single `fetch(&SyncState)` method (mixed sync/async) that pages forward from a `*_forward` cursor. The `cursors` table already stores both `newest_ts` and `oldest_ts` per channel and `update_window` maintains `MIN(oldest)`/`MAX(newest)`, but only `newest_ts` is ever read — the retroactive direction was designed for and left unwired. `ExtractStage` regex-extracts `linear:AB-123` and `mr:!NNNN` into `Record.entities`; `CrossSourceDedupeStage` merges records sharing an entity; `EnrichStage` maps entities/channels to projects via `EntityRegistry`, which already exposes `project_for_repo`, `project_for_linear`, and `resolve_person`. Record IDs are deterministic (`linear-AB-123`, `git-<repo>-<hash8>`), and `SyncState::get_hash(id)` answers "have we seen this record" cheaply.

Two hard constraints:
1. `PipelineStage` is a **sync** trait (`process(Vec<Record>) -> Vec<Record>`); it cannot perform async I/O, so the enrich→fetch→re-enrich loop cannot be a stage.
2. `LinearFetcher` fetches only `viewer.assignedIssues`, so referenced tickets you don't own have no record to link to.

This change builds on `unify-fetchers` (kb-fetchers is the single source of truth for external data). There is currently no GitLab fetcher; its old backend counterpart was deleted by `unify-fetchers`.

## Goals / Non-Goals

**Goals:**
- A uniform two-mode fetcher contract: forward updates + on-demand (targeted + retroactive).
- A bounded resolver loop that closes entity references by pulling missing data on demand.
- First-class GitLab MR ingestion that links to Linear tickets and commits.
- Rich git records: changed files, commit URLs, repo→project→Linear linking.
- Reliable commit/MR ↔ Linear ticket links regardless of who owns the ticket.

**Non-Goals:**
- Changing the TUI feed rendering or the `.feeds/` format (owned by `unify-fetchers`).
- Tree-sitter / code-symbol indexing (the config comment is aspirational; out of scope).
- Storing full diffs or file contents (only the changed-file list and structural metadata).
- Resolving people identity inside the loop for v1 (work-item entities only; see Open Questions).
- Replacing `vault_manager.rs` or the embedding/model machinery.

## Decisions

### 1. A `Fetcher` trait with explicit modes rather than one overloaded method
```rust
#[async_trait]
pub trait Fetcher {
    fn source(&self) -> Source;
    async fn fetch_updates(&self, state: &SyncState) -> Result<Vec<Record>>;
    async fn fetch_targets(&self, refs: &[EntityRef]) -> Result<Vec<Record>> { Ok(vec![]) }
    async fn fetch_backfill(&self, state: &SyncState, budget: Budget) -> Result<Vec<Record>> { Ok(vec![]) }
    fn resolves(&self) -> &'static [&'static str] { &[] } // e.g. ["linear"], ["mr"]
}
```
Default impls mean vault/calendar/gdocs need not implement on-demand modes. **Rationale**: modes are semantically distinct (forward vs by-id vs backward) and the router needs `resolves()` to dispatch. **Alternative considered**: a single `fetch(mode: Mode)` enum-dispatch method — rejected because targeted needs `&[EntityRef]` and backfill needs a budget, forcing an awkward union parameter. **Alternative**: keep bare structs and special-case in the resolver — rejected; the resolver would hardcode every fetcher.

### 2. `EntityRef` parsed from existing entity strings; router keyed on `resolves()`
`EntityRef { kind: EntityKind, id: String, project: Option<String> }` parsed from `linear:AB-123` / `mr:!2935`. **Rationale**: reuses the vocabulary `ExtractStage` already produces. The `project` field carries repo/namespace context for MR IIDs (unique only per project). **Alternative**: full URLs as refs — rejected as lossy for the bare `!NNNN` mention form.

### 3. Resolver is an async orchestrator in `kb-sync`, not a `PipelineStage`
The pipeline splits into **pre-stages** (normalize, filter, dedupe, extract) → **resolve** (async fixpoint) → **post-stages** (cross_dedupe, chunk, summarize, enrich). **Rationale**: the sync `PipelineStage` trait cannot fetch; cross_dedupe must run *after* resolution so a pulled ticket can merge with its referencing commit/MR. **Alternative**: make `PipelineStage` async — rejected as a large blast-radius change to every stage for one consumer.

Loop shape:
```
records = pre_stages(seed)
frontier = entity_refs(records); visited = {}
for _ in 0..max_depth:
    missing = frontier - visited - already_in_kb(id_of(ref))
    if missing.is_empty(): break
    fetched = route(missing).fetch_targets(...)   // grouped per source
    visited += missing
    new = pre_stages(fetched); records += new
    frontier = entity_refs(new)
post_stages(records)
```
Bounded by `max_depth` (default 2) and a per-run fetch budget; `already_in_kb` uses the deterministic-id hash check. Targeted records pass the normal hash filter, so re-resolution is idempotent.

### 4. GitLab via `glab` CLI, project path derived from the `origin` remote
**Rationale**: reuses existing `glab` auth and matches the binary-shell pattern of `linear`/`gmail` fetchers; keeps `[[repos]]` the single source of truth (no new config). **Alternative**: GitLab GraphQL + PAT — more portable but adds a secret and duplicates auth; deferred.

### 5. Distinct `Source::Gitlab` + `Kind::Merge`
**Rationale**: enables `--source gitlab` filtering and dedicated feed handling. **Trade-off**: touches every `match` on `Source`/`Kind` (record, parquet_store). Small and mechanical.

### 6. Changed files in `content` for v1 (no schema change)
Append a "Changed files" block to commit `content`. **Rationale**: immediately searchable, backward-compatible (Parquet read tolerates missing columns). **Alternative**: a typed `files: Vec<String>` column — deferred to a follow-up once structured file queries are needed.

### 7. Linear targeted fetch by identifier
`fetch_targets` batch-queries `issues(filter: { number: { in: [...] } })` grouped by team prefix, so referenced-but-unassigned tickets get records. **Rationale**: closes the ownership gap that otherwise breaks linking.

### 8. Normalize lowercase Linear IDs from MR branches/titles
Linear's GitLab integration names branches `user/ab-123-slug`. `ExtractStage` uppercases matches from `source_branch`/title before emitting `linear:AB-123`, and prefers the MR-URL form (which carries the project) over bare `!NNNN`.

## Risks / Trade-offs

- [Risk] Resolver crawls unbounded → **Mitigation**: `max_depth` + fetch budget + `visited` set keyed on normalized ref.
- [Risk] Targeted API calls hit rate limits → **Mitigation**: batch per source (Linear `number.in`, grouped glab calls); reuse kb-fetcher retry/backoff.
- [Risk] Bare `mr:!NNNN` mentions can't be routed to a project → **Mitigation**: resolve only when a project is known (URL form or single-repo config); leave bare mentions best-effort and logged.
- [Risk] Async-trait migration touches all fetchers → **Mitigation**: land the trait + refactor as an isolated first task with `cargo check` gate before the resolver depends on it.
- [Risk] `glab` absent or unauthenticated in some environments → **Mitigation**: fetcher degrades to empty result with a warning, matching existing fetcher failure handling (one source failing never aborts the cycle).
- [Trade-off] Files-in-content is unstructured → accepted for v1; typed column is a clean follow-up.
- [Risk] Repo→project linking depends on vault frontmatter → **Mitigation**: ship an audit of `1. Projects/*.md` missing `gitlab_repos`/`linear_prefix`.

## Migration Plan

1. Land the `Fetcher` trait + refactor existing fetchers onto `fetch_updates` (behavior-preserving); `cargo check`.
2. Add `Source::Gitlab`/`Kind::Merge` + storage arms.
3. Add git enhancements (files, url, enrich) — independently shippable value.
4. Add the GitLab fetcher.
5. Add `EntityRef`/router + resolver + pipeline pre/resolve/post split.
6. Add Linear `fetch_targets` + `SyncState::oldest_cursor` + `kb backfill`.
7. Wire `git`/`gitlab` into the fetch cycle.

Rollback: the resolver is opt-in behind a max_depth=0 short-circuit (no targeted fetches); disabling it reverts to forward-only behavior without code removal.

## Open Questions

- Resolver depth: fixed default 2, or per-run configurable? (Leaning: config with default 2.)
- Should the resolver also resolve people (`author → People/` identity) in v1, or ship work-items only first?
- Is Linear's native GitLab integration enabled? If so, issue `attachments { url }` gives an authoritative MR↔issue edge that could reduce reliance on branch/message parsing.
