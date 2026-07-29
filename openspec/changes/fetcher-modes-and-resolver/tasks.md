## 1. Data model + storage

- [ ] 1.1 Add `Source::Gitlab` (Display `"gitlab"`) and `Kind::Merge` (`"merge"`) in `crates/kb-core/src/record.rs`
- [ ] 1.2 Add matching arms in `parquet_store.rs`: `source_str`, `parse_source`, `kind_str`, `parse_kind`
- [ ] 1.3 Add `SyncState::oldest_cursor(channel)` reader (reads existing `oldest_ts`) in `crates/kb-core/src/sync_state.rs`
- [ ] 1.4 `cargo check` the workspace

## 2. Fetcher trait + refactor (behavior-preserving)

- [ ] 2.1 Define `Fetcher` trait (`fetch_updates`, default `fetch_targets`/`fetch_backfill`, `resolves`, `source`) and `Budget` type in `crates/kb-fetchers/src/lib.rs`
- [ ] 2.2 Define `EntityRef { kind, id, project }` + parser from entity strings (`linear:*`, `mr:!*`) and a source `Router`
- [ ] 2.3 Refactor each existing fetcher (slack, gmail, linear, git, vault, calendar, gdocs) to implement the trait, renaming current `fetch` → `fetch_updates` (no behavior change)
- [ ] 2.4 Update `kb_sync::fetch_with_lookback` dispatch to call `fetch_updates` via the trait
- [ ] 2.5 `cargo check` — confirm forward fetching is unchanged

## 3. Git activity enrichment

- [ ] 3.1 Extend `git_log` in `git.rs` with `--name-status` and a leading `\x01` record marker; parse changed files per commit
- [ ] 3.2 Append a "Changed files" section to each commit record's content
- [ ] 3.3 Add origin-remote normalization (SSH/HTTPS → canonical base) and populate commit `url`
- [ ] 3.4 In `enrich.rs`, resolve git records' repo via `EntityRegistry::project_for_repo` → add `project:` and `linear-prefix:` tags (add a `linear_prefix_for_repo` helper on the registry)
- [ ] 3.5 Add `git` to the fetch cycle sources and completed-set loop in `fetch_cycle.rs`

## 4. GitLab fetcher

- [ ] 4.1 Create `crates/kb-fetchers/src/gitlab.rs`: `GitlabFetcher` deriving project path from origin remote; `mod gitlab;` in `lib.rs`
- [ ] 4.2 Implement `fetch_updates` via `glab mr list --repo <path> --updated-after <cursor> -F json`; map MRs → `Record { source: Gitlab, kind: Merge, url, author, tags }`
- [ ] 4.3 Extract Linear IDs from MR `source_branch`/title with lowercase normalization; prefer MR-URL form carrying project context
- [ ] 4.4 Implement `fetch_targets` for `mr` refs that carry project + IID; set `resolves()` to `["mr"]`
- [ ] 4.5 Add `"gitlab"` dispatch arm and to `ALL_SOURCES` in `kb-sync`; add `gitlab` to the fetch cycle
- [ ] 4.6 Degrade to empty result + warning when `glab` is missing/unauthenticated

## 5. Resolver + pipeline phase split

- [ ] 5.1 Factor extraction regex into shared `entity_refs(&Record) -> Vec<EntityRef>` reused by `ExtractStage` and the resolver
- [ ] 5.2 Split `kb_sync::index` into pre-stages (normalize, filter, dedupe, extract) → resolve → post-stages (cross_dedupe, chunk, summarize, enrich)
- [ ] 5.3 Implement the async resolver loop: missing = frontier − visited − already_in_kb (deterministic-id hash check); route + `fetch_targets`; re-run pre-stages; iterate to `max_depth` (default 2) with a fetch budget
- [ ] 5.4 Add `max_depth`/budget config with a `max_depth = 0` short-circuit (no targeted fetches) for rollback

## 6. Linear targeted + retroactive backfill

- [ ] 6.1 Implement `LinearFetcher::fetch_targets` via `issues(filter: { number: { in: [...] } })` grouped by team prefix; set `resolves()` to `["linear"]`
- [ ] 6.2 Implement `fetch_backfill` for git (walk older history) and Linear/GitLab (page older than `oldest_cursor`), extending the oldest cursor via `update_window`
- [ ] 6.3 Add a `kb backfill --source <s> [--until <date>]` command driving `fetch_backfill` under a budget

## 7. Verify

- [ ] 7.1 `cargo check` + `cargo clippy` clean across the workspace
- [ ] 7.2 Unit test: `EntityRef` parse/route, remote-URL normalization, name-status parsing, lowercase branch ID extraction
- [ ] 7.3 Unit test: resolver fixpoint terminates at depth cap and skips already-known ids
- [ ] 7.4 Manual: run a fetch cycle with `git`+`gitlab` enabled; confirm commit/MR records have URLs, changed files, and link to Linear tickets (including one not assigned to the user)
- [ ] 7.5 Audit `1. Projects/*.md` for missing `gitlab_repos`/`linear_prefix` frontmatter; report the list
- [ ] 7.6 `openspec validate fetcher-modes-and-resolver --strict`
