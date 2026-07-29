## Why

Pasta's repo view is commit-message-only and one-directional: fetchers page forward from a cursor, the sync-time enricher can only link records to data that already happens to be in the KB, and there is no GitLab fetcher at all (MR signal arrives incidentally through Gmail notifications). As a result, a commit or MR that references a Linear ticket you don't own cannot be linked, changed files and commit URLs are never captured, and there is no way to backfill history on demand. Making fetchers demand-driven (not just forward) and giving the enricher the ability to pull the specific data it is missing turns fragmented per-source records into a connected graph of commits ↔ MRs ↔ Linear tickets.

## What Changes

- Introduce a `Fetcher` trait with two modes: **updates** (forward, cursor-based — today's behavior) and **on-demand** (enricher-driven), the latter split into **targeted** (`fetch_targets` — resolve specific entities by reference) and **retroactive** (`fetch_backfill` — page older than the stored oldest cursor). Migrate all seven kb-fetchers onto the trait.
- Add an **entity resolver**: an async fixpoint loop that extracts referenced entities from records, fetches the ones missing from the KB via targeted mode, re-extracts, and repeats until closure (bounded by max depth and a fetch budget). This is the "enricher collects missing data → fetches → enriches → refetches" loop.
- Add a **GitLab fetcher** (`kb-fetchers/src/gitlab.rs`) using the `glab` CLI, deriving each project path from the repo's `origin` remote. Emits MR records (`Source::Gitlab`, `Kind::Merge`) with author, state, URL, and branch.
- Enhance the **git fetcher**: capture changed files (`--name-status`), populate commit `url` from the origin remote, and enrich commits with `project:` / `linear-prefix:` tags via the existing `EntityRegistry::project_for_repo`.
- Extend Linear resolution so referenced issues (not just `viewer.assignedIssues`) can be fetched by identifier, closing the "ticket not assigned to me" gap.
- Split the ingestion pipeline into **pre-stages → resolve → post-stages** so cross-source dedup runs *after* on-demand fetches, letting a freshly pulled ticket merge with the commit/MR that referenced it.
- Expose `SyncState::oldest_cursor` (the `oldest_ts` column already exists) and a `kb backfill` command to drive retroactive fetches.
- **BREAKING** (internal): each fetcher's bare `fetch(...)` method is renamed/reshaped to `fetch_updates(...)` behind the new trait; `kb_sync::fetch` dispatch is updated accordingly. No user-facing config breakage.

## Capabilities

### New Capabilities
- `fetcher-modes`: A common `Fetcher` trait exposing forward `fetch_updates`, targeted `fetch_targets`, and retroactive `fetch_backfill` modes, with an `EntityRef` type and a source router.
- `entity-resolver`: A bounded async reference-closure loop that pulls entities missing from the KB on demand and feeds them back through extraction until no new references remain.
- `gitlab-fetch`: A GitLab merge-request fetcher over the `glab` CLI that produces linkable MR records and resolves MRs by reference.
- `repo-activity`: Git commit records enriched with changed files, commit URLs, and repo→project→Linear-prefix linking.

### Modified Capabilities
- `ingestion-pipeline`: Pipeline execution splits into pre-stages, an async resolve phase, and post-stages; cross-source dedup is guaranteed to run after on-demand fetches.
- `state-persistence`: `SyncState` exposes the oldest cursor so fetchers can page retroactively and resume backfill.

## Impact

- `crates/kb-fetchers/src/` — new `gitlab.rs`; `git.rs`, `linear.rs`, `slack.rs`, `gmail.rs`, `calendar.rs`, `gdocs.rs`, `vault.rs` refactored onto the `Fetcher` trait (+ `fetch_targets`/`fetch_backfill` where applicable); new `lib.rs` trait + `EntityRef`/router.
- `crates/kb-core/src/record.rs` — add `Source::Gitlab` and `Kind::Merge`.
- `crates/kb-core/src/sync_state.rs` — add `oldest_cursor()`.
- `crates/kb-pipeline/src/` — factor extraction into a shared `entity_refs()`; `enrich.rs` gains git repo→project→Linear enrichment; new `resolver` orchestrator; `extract.rs` prefers MR-URL form and normalizes lowercase branch IDs.
- `crates/kb-storage/src/parquet_store.rs` — `source_str`/`parse_source`/`kind_str`/`parse_kind` arms for the new variants.
- `crates/kb-sync/src/lib.rs` — dispatch `"gitlab"`, add to `ALL_SOURCES`, run pre/resolve/post phases in `index`.
- `crates/kb-cli` / `crates/backend/src/fetch_cycle.rs` — add `git`/`gitlab` to the cycle; new `kb backfill` command.
- Config: `[binaries].glab` already exists; `1. Projects/*.md` need `gitlab_repos:` and `linear_prefix:` frontmatter for repo linking (audit, not code).
- Depends on and layers over the `unify-fetchers` change (kb-fetchers as single source of truth).
