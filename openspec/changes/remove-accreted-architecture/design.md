## Context

pasta is 9,703 lines across 12 crates. The crate split is sound and clippy is nearly silent under pedantic+nursery, but the tree holds three coexisting generations of architecture: the retired `history/` LanceDB index, the standalone `kb` engine invoked as a subprocess, and the current in-process `kb-sync` library. Each migration landed as addition without deletion.

The evidence is measurable. `~/.kb` contains `state.db` (current), `sync_state.db` (0 bytes, previous generation), and `sync_state.json` (the one before that). `crates/search` opens `<vault>/.lancedb`, a store deleted in the history migration. `tests/wiki_e2e.rs` imports no crate under test, requires a live Ollama, and writes into the live vault at two retired paths. Two promoted specs (`roadmap-health`, `relevance-feedback`) have zero implementing code, and four more contain requirements naming deleted files.

Two defects are not merely cosmetic. `Command::ForceFetch` starts a fetch/index cycle without the scheduler's `running_native` marker, so the manual key can run a second concurrent cycle over four stores. And `load_tasks` reads the task directory non-recursively while default-enabled vault maintenance moves task files into `Tasks/<Initiative>/`, so routed tasks silently disappear from the TUI and the state snapshot while the Trello sync (which walks recursively) keeps acting on them.

There is now a git history — a single initial commit — so this work is diffable and revertable for the first time.

## Goals / Non-Goals

**Goals:**
- Remove code, specs, and documentation that describe or serve deleted architecture
- Leave exactly one notion of "the set of tasks" and one notion of "where the knowledge base lives"
- Make the kb boundary coherent: library-only inside the daemon
- Close the concurrent-fetch corruption window
- Centralise vault paths so the next divergence is a compile error rather than a silent empty read
- Produce the evidence needed to decide the storage question, without deciding it here

**Non-Goals:**
- Consolidating the four storage engines. 1,072 records and 780 KB of JSON are served by 12 MB of index across LanceDB, Tantivy, Parquet and SQLite, which looks indefensible — but the daemon currently never indexes the `vault` source, so nobody knows what semantic search over the vault is worth. That decision belongs to a follow-up change, informed by task 6 here.
- Fixing the indirect prompt-injection exposure (verbatim external content read by `--trust-all-tools` agents). Real, and a separate change.
- The daemon's client lifecycle bugs (writer task dying permanently, no TUI reconnect, orphaned ACP children). Separate change.
- Implementing `roadmap-health` or `relevance-feedback`. Their change proposals stay in `openspec/changes/` as unstarted work; only the prematurely promoted main specs are deleted.
- `state-persistence`'s "Log cleanup uses absolute path" requirement stays unimplemented and unmodified. It is a genuine pending item, not spec drift.

## Decisions

**Delete `crates/mcp`, keep `crates/kb-mcp`.** Two MCP servers expose knowledge-base search; neither is registered anywhere in the repo. `kb-mcp` is the one documented in `KB_README.md` and the one whose name matches the engine. `crates/mcp` additionally has the weaker implementation — it calls `hybrid_search` with no over-fetch and then filters, so it silently returns fewer results than requested. Its three unique tools (`search_tasks`, `get_feeds`, `get_people`) are thin `pasta_common::vault` wrappers; if they are wanted later they belong in `kb-mcp` or a new pasta-scoped server, not in a crate resurrected for them. Alternative considered: keep `crates/mcp` and delete `kb-mcp` — rejected because `kb-mcp` is smaller (63 lines), correct, and already referenced by the Dockerfile.

**Make `load_tasks` recursive rather than stopping the router.** The router exists because a flat `Tasks/` with dozens of files is unusable in Obsidian, and initiative subfolders are the organising principle the vault already uses. Recursion is the smaller change and preserves both behaviours. This requires `Task` to carry the path it was loaded from instead of a bare file name, because `rewrite_status` and `update_task_priority` currently rebuild `<task dir>/<file>` and would silently write to the wrong location for subfolder tasks. Alternative considered: stop the router — rejected, it discards a feature the user relies on.

**Route the two `kb` subprocess sites to the library.** `Backfill` shells out to `kb sync`, which is itself only `kb_sync::run` — the same function `fetch_cycle` already calls in-process. Inbox processing shells out to `kb recent --days N`, which is `ParquetStore::read_all` plus a date filter. Both become direct calls. This also removes a class of silent failure: `Backfill` currently discards the subprocess result and unconditionally flashes "kb sync complete", and the `--json` flag inbox processing passes does not exist on `kb recent`, so that path fails on argument parsing even when `kb` is installed. `crates/kb-cli` survives as a user-facing CLI; it simply stops being load-bearing for the daemon. Alternative considered: keep the subprocess boundary and resolve `kb` through `resolve_binary` — rejected, it preserves process-spawn cost and stderr-parsing for no isolation benefit inside one workspace.

**One `KbConfig` resolution, exported from `kb_search`.** `[kb] data_dir` is honoured at exactly one read site and ignored at six sites that call `KbConfig::default()`, so a non-default value points the daemon's search at a store nothing writes. The existing resolver in `backend/src/kb_search.rs` moves to `pasta_common` (or is re-exported) and becomes the only constructor callers use. Alternative considered: delete the config key — rejected, per-machine data location is legitimate; the bug is having two answers, not having the option.

**`VaultLayout` as a struct of resolved `PathBuf`s, constructed once from config.** Methods, not constants, so the type can log a warning when a directory is missing instead of returning an empty `read_dir`. Path literals appear ~20 times for `"Tasks"` alone and 5 times for the daily-note path, and `.feeds` is vault-relative in `common` while `kb-sync` hardcodes `/home/orre/Obsidian/Readpeak/.feeds`. Alternative considered: `const` strings in one module — rejected, it removes the duplication but not the silent-empty-read failure mode.

**Sequence deletions before behaviour changes.** Phases 1–2 remove code and specs with no behaviour change, so the diff is reviewable as pure subtraction and any regression in later phases is unambiguously attributable. `ForceFetch` is the exception: it lands in phase 1 despite being a behaviour change, because it is one line and the only item with live corruption exposure.

## Risks / Trade-offs

[Deleting `crates/mcp` removes tools someone configured outside the repo] → No MCP registration exists in `.kiro/` and nothing builds the binary; the three unique tools are named in the proposal so they can be reintroduced deliberately.

[Recursive `load_tasks` surfaces a backlog of previously hidden tasks, and the list gets longer] → That is the bug being fixed, not a side effect. The TUI already supports filtering, and the count change is called out in the proposal so it is not mistaken for a regression.

[`Task` gaining a path field changes an IPC wire type] → Backend and TUI are always built and shipped together by `start.sh`; there is no independent client. A stale TUI binary against a new daemon would fail to deserialize, which surfaces immediately rather than silently.

[Inbox processing moving in-process loses the memory isolation of a subprocess] → `ParquetStore::read_all` already runs in-process in the same daemon for the PARA audit and semantic routing, so the ceiling is unchanged. `run-sync.sh` caps the daemon at 28 GB.

[Full-corpus `read_all` for a 2-day inbox window is wasteful] → True today via the subprocess as well; this change preserves current behaviour rather than fixing it. Task 6 records the cost so the storage change can address it with data.

[Rewriting three specs while deleting two could lose intent someone still wants] → Deleted specs are only the two with zero implementation, and their change proposals remain. `REMOVED Requirements` entries carry reason and migration notes, so archive history explains each removal.

[A large mechanical diff hides a real behaviour change] → Each phase lands as its own commit with `cargo build` plus clippy clean, and phases 3–5 add tests for the pure functions they touch before changing them.

## Migration Plan

1. Phase 1 (deletion + the one-line guard): mechanical, no behaviour change beyond `ForceFetch`.
2. Phase 2 (specs and docs): brings `openspec/specs/` and `KB_README.md` in line with the code.
3. Phase 3 (task set): recursive enumeration, `Task` carries its path, consumers unified.
4. Phase 4 (kb boundary): subprocess sites replaced, single `KbConfig` resolution.
5. Phase 5 (`VaultLayout`): mechanical replacement of literals, one crate at a time.
6. Phase 6 (evidence): record what each store contributes, to unblock the storage change.

Rollback is per-phase `git revert`. No data migration: no on-disk format changes, and `~/.kb` is untouched. Users see no config change; `[kb] data_dir` starts being respected by writers, which for anyone on the default is a no-op.

## Open Questions

- Should `crates/mcp`'s `search_tasks` / `get_feeds` / `get_people` be reintroduced in `kb-mcp` in phase 1, or left out until something actually consumes them? Leaning: leave out.
- Do the leftover `~/.kb/sync_state.db` and `sync_state.json` files get deleted as part of phase 1, or left as inert artifacts? Leaning: delete, and note it in the phase commit.
- Is `kb-cli` still wanted at all once the daemon stops using it? It is the only way to run `reindex` and `reprocess`, so probably yes — but that answer may change with the storage decision.
