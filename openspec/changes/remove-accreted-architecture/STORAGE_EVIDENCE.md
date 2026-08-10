# Storage evidence

Task 6 of `remove-accreted-architecture` (tasks.md 6.1–6.5). This document is
evidence, not a decision: it records which code paths use which of the four
storage engines, whether the `vault` source is actually indexed, and what
retrieval quality/latency and full-corpus-read cost look like on the real
corpus today. Consolidating the four engines is explicitly a **follow-up
change** (see the recommendation at the end), not something this document or
this change decides.

Measurements were taken against the live `~/.kb` store (`state.db`, Parquet
under `raw/`, Tantivy under `index/`, LanceDB under `vectors/`) on 2026-08-10,
using a release build. The corpus at measurement time: 1,147 records across
gmail (583), vault (225), slack (200), calendar (80), git (33), linear (17),
gdocs (9).

## 6.1 — Which code path queries which store

| Store | Code path | Call sites |
|---|---|---|
| Tantivy (full-text) | `crates/kb-storage/src/text_index.rs` (`TextIndex`) | Opened/queried only inside `hybrid.rs`'s `hybrid_search` (`TextIndex::open` + `.search()`), and rebuilt by `kb-cli`'s `reindex`/`reprocess` (`TextIndex::clear`). No caller queries Tantivy standalone in production code — `hybrid_search` is the only read path. |
| LanceDB (vectors) | `crates/kb-storage/src/vector_store.rs` (`VectorStore`) | Same shape: opened/queried only inside `hybrid.rs`'s `hybrid_search` (`VectorStore::new` + `.search()`), and rebuilt by `kb-cli`'s `reindex`/`reprocess`. No standalone vector-only read path exists in production code either. |
| Parquet (raw records) | `crates/kb-storage/src/parquet_store.rs` (`ParquetStore`) | Two distinct usages: (a) **write path** — `kb_sync::run`/`kb_sync::index` write every synced record here, the only durable copy of raw content; (b) **direct `read_all` reads**, bypassing both indexes entirely: `crates/backend/src/inbox.rs` (recent-record scan for inbox processing) and `crates/backend/src/vault_organize.rs` (PARA audit, at line 129 `ParquetStore::new(&cfg).read_all()`). `kb-cli`'s `Recent`, `Reindex`, and `Reprocess` subcommands also call `read_all` directly. |
| SQLite (`state.db`) | `crates/kb-core/src/sync_state.rs` (`SyncState`) | Per-channel fetch cursors (`cursors` table, incremental fetch windows), per-record content hashes (`content_hashes`, dedup/skip-unchanged), and the locked embedding model name (`meta`). Read/written by `kb_sync` during every fetch cycle and by `kb-cli reindex`/`reprocess` to re-lock the model. Not queried for search at all — it is bookkeeping, not a content store. |

All *search* (as opposed to raw-read) traffic funnels through exactly one
function: `hybrid_search` in `crates/kb-storage/src/hybrid.rs`, which queries
Tantivy and LanceDB in parallel and merges with Reciprocal Rank Fusion (RRF,
k=60). `crates/backend/src/kb_search.rs::search` wraps that with pasta's
over-fetch-then-filter (source/date/participant) for the TUI and MCP. There
is no code path in the backend that queries Tantivy or LanceDB alone; the
only "Tantivy-only" numbers in this document come from a throwaway benchmark
written for measurement, not from production code.

On disk: `index/` (Tantivy) is 5.6 MB, `vectors/` (LanceDB) is 8.8 MB,
`state.db` is 592 KB, and `raw/` (Parquet) is 2.1 MB across 217 files, for
1,147 records / roughly 780 KB of raw JSON-equivalent content. A separate,
older LanceDB store also exists at `<vault>/.lancedb` (321 MB) — that is the
retired `history/` index this change's design doc already identifies as dead
(no code under the current architecture opens `<vault>/.lancedb`; only
`crates/search`, deleted in phase 1, ever did).

## 6.2 — Is the `vault` source indexed?

**No, not by the automated fetch cycle.** `crates/backend/src/fetch_cycle.rs`
line 12 hardcodes:

```rust
let sources = &["gmail", "linear", "calendar", "slack"];
```

`vault` is absent. `kb_sync::ALL_SOURCES` (`kb-sync/src/lib.rs`) and the
manual `Backfill` command's source list (`commands.rs` line 91) both include
`"vault"`, but nothing calls the automated cycle with it — only a one-off
`kb sync` or `kb-cli sync -s vault` invocation would.

The 225 `vault`-source records currently in Parquet are real but **stale**:
every `raw/vault/**/*.parquet` file's mtime is 2026-06-27 through 2026-06-29.
Nothing has written a vault record in the six weeks since (measured against
2026-08-10). The vault content in the index today reflects the vault as it
was in late June.

This matters because two consumers filter on `source == "vault"` and are
silently running against that stale snapshot rather than being silently
empty:

- `crates/backend/src/vault_organize.rs` line 88, the **PARA audit**
  (`audit_para`), filters `hybrid_search` results to `source == "vault"` to
  find mis-filed notes.
- `crates/backend/src/vault_organize.rs` line 145, the **daily note
  backlinks** step, calls `kb_search::search(&summary, Some("vault"), ...)`.
- `crates/backend/src/fetchers/vault_manager.rs` line 503,
  **`route_new_tasks_semantic`**, calls the same `kb_search::search(...,
  Some("vault"), ...)` to find semantically related existing tasks before
  routing a new one.

None of these fail or error — they return plausible-looking results drawn
from a six-week-old vault snapshot, which is worse than an obvious failure.
This change does not add `vault` to the automated fetch cycle: doing so
changes fetch cost and cadence for three consumers at once and deserves its
own change with its own before/after measurement, not a one-line addition
buried in an evidence-gathering task. **Recorded and left as-is, called out
explicitly in the follow-up recommendation below.**

## 6.3 — Latency and quality: Tantivy-only vs hybrid

Measured with a release build against the real corpus above, same five
queries chosen from terms that actually appear in the indexed content
(`meeting`, `deploy`, `pipeline`, `review`, `standup`), limit 10, index
warmed (first call to each excluded):

| Query | Tantivy-only | Hybrid (vector + full-text, RRF) | Hybrid-only results (not in Tantivy's top 10) |
|---|---|---|---|
| `meeting` | 10 hits, 7.1 ms | 10 hits, 269.0 ms | 3 |
| `deploy` | 10 hits, 2.9 ms | 10 hits, 202.3 ms | 6 |
| `pipeline` | 10 hits, 4.9 ms | 10 hits, 202.3 ms | 7 |
| `review` | 10 hits, 2.8 ms | 10 hits, 200.1 ms | 4 |
| `standup` | 0 hits, 3.5 ms | 10 hits, 207.3 ms | 10 |

Tantivy-only latency is consistently single-digit milliseconds. Hybrid is
roughly 30–70× slower in absolute terms (200–270 ms), almost entirely spent
in the embedding call for the query vector (`embedder::embed_batch`/
equivalent single-query embed), not in LanceDB's own read.

Quality: hybrid surfaces results Tantivy's lexical match misses in every
query tested, most starkly `standup` — a term Tantivy found zero matches for
(no document contains that exact token) where the vector search still
returned 10 semantically related hits. For `meeting`/`review`/`deploy`, 3–7
of the 10 hybrid results were not in Tantivy's own top 10, i.e. RRF's
re-ranking plus the vector leg change results even when Tantivy does find
matches, not just when it finds none.

This is evidence for, not against, keeping the vector leg: the latency cost
is real and attributable almost entirely to embedding the query, and the
quality gain (particularly the zero-lexical-match case) is exactly what a
hybrid design is for. It does not by itself justify keeping LanceDB as a
persistent 8.8 MB on-disk index rather than, say, an in-memory vector store
rebuilt from Parquet at startup — that trade-off is for the follow-up change.

## 6.4 — Cost of full-corpus `read_all`

`ParquetStore::read_all()` walks all 217 files under `raw/` and deserializes
every Arrow batch into `Record` structs. Measured cold (first call) and warm
(subsequent calls, OS page cache populated):

- 1,147 records read from 217 Parquet files in **17–100 ms** across repeated
  runs (31.8 ms, 17.9 ms, 17.5 ms warm; 99.4 ms on the first/cold call).

This is called by two production code paths on every invocation, with no
caching and no date-range pushdown at the file level — the date filter in
both is applied in-memory *after* every record has already been fully
deserialized:

- `crates/backend/src/inbox.rs` line 30 — reads **all 1,147 records** to find
  the ones from the last 2 days.
- `crates/backend/src/vault_organize.rs` line 129 — reads **all 1,147
  records** for the PARA audit's `source == "vault"` filter (§6.2).

At the current corpus size (1,147 records, 2.1 MB raw, 217 files) this cost
is negligible in absolute terms — under 100 ms even cold — and is not a
practical performance problem today. It is, however, `O(total records)` work
for what both callers actually need (`O(records matching a filter)`), and
that gap grows linearly with corpus size and file count. The 780 KB /
1,072-record figure in the design doc's non-goals section is consistent with
what full-corpus reads cost today: cheap, but only because the corpus is
still small.

## 6.5 — Findings and follow-up recommendation

**What each store is for, in practice, today:**
- Parquet is the only store with a direct non-search read path in production
  code (`inbox.rs`, `vault_organize.rs`), and the only one that is genuinely
  the source of truth — LanceDB and Tantivy are both derived and rebuildable
  from it via `kb-cli reindex`.
- Tantivy and LanceDB have exactly one production caller each:
  `hybrid_search`. Neither is ever queried standalone outside of this
  evidence-gathering benchmark.
- SQLite (`state.db`) is bookkeeping (cursors, hashes, locked model), not a
  content store, and was never a candidate for consolidation with the other
  three.
- The `vault` source is real but stale (six weeks old as of this writing)
  because the automated fetch cycle omits it. Three consumers
  (`audit_para`, the daily-note backlinks step, `route_new_tasks_semantic`)
  silently run against that stale snapshot rather than visibly failing.

**What the measurements support:**
- Hybrid search's latency cost (200–270 ms vs. 3–7 ms) is real and dominated
  by query embedding, not by LanceDB's read itself.
- Hybrid search's quality gain over Tantivy-only is also real and
  reproducible on this corpus — every tested query, hybrid returned results
  Tantivy's lexical match did not surface.
- Full-corpus `read_all` is cheap today (under 100 ms) but is `O(total
  records)` where its two callers need `O(matching records)`, and that gap
  is currently masked by corpus size, not solved.

**Recommendation for the follow-up change** (not decided here):
1. Decide whether to add `vault` to the automated fetch cycle, given the
   three consumers that currently run against a stale snapshot without any
   visible sign of it. If yes, measure the added fetch cost/cadence
   separately, since vault content changes on every note edit, unlike
   gmail/linear/calendar/slack's API-polling cadence.
2. Given hybrid search's measured latency and quality trade-off, decide
   whether LanceDB should remain a persistent on-disk index or become an
   in-memory structure rebuilt from Parquet at startup — the corpus (1,147
   records, 8.8 MB of vectors) is small enough that either is plausible, and
   this document's numbers are the input to that call, not the call itself.
3. If full-corpus `read_all` costs grow (larger corpus, more files), give
   `inbox.rs` and `vault_organize.rs`'s date/source filters file-level
   pushdown (skip files outside the date partition, as the write-side
   partitioning by `source/month` already supports) rather than
   deserializing every record and filtering in memory.

This change does not implement any of the three items above. It stops at
recording the evidence a follow-up change needs to decide them.
