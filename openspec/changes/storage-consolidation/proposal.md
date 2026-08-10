## Why

`remove-accreted-architecture` (task 6, `STORAGE_EVIDENCE.md`) measured how
pasta's four storage engines — Tantivy, LanceDB, Parquet, and SQLite — are
actually used, without deciding whether to consolidate them. That change
explicitly scoped consolidation as a non-goal: 1,147 records and about 780 KB
of raw JSON-equivalent content are served by roughly 17 MB of index
(Tantivy 5.6 MB, LanceDB 8.8 MB, `state.db` 592 KB, Parquet 2.1 MB), which
looks disproportionate on its face but needed evidence before anyone could
act on that impression. The evidence now exists. This proposal carries it
forward into a recommendation and asks for a decision.

## What Changes

This proposal does not touch code. It reviews `STORAGE_EVIDENCE.md`'s
findings and asks the maintainer to decide three open questions before any
implementing change is written:

1. **Index the `vault` source in the automated fetch cycle, or don't, but
   stop it being silent.** `fetch_cycle.rs` hardcodes
   `["gmail", "linear", "calendar", "slack"]`; `vault` is absent. The 225
   `vault` records already in Parquet are six weeks stale as of the
   evidence date, and three consumers (`audit_para`, the daily-note
   backlinks step, `route_new_tasks_semantic`) filter on `source == "vault"`
   and silently return plausible-looking results from that stale snapshot.
   Recommendation: **add `vault` to the automated fetch cycle**, because the
   failure mode today is not "semantic routing over the vault doesn't work"
   but "semantic routing over the vault looks like it works and is quietly
   wrong," which is worse than a visible gap. This needs its own
   before/after measurement of fetch cost and cadence, since vault content
   changes on every note edit rather than on an API-polling schedule like
   the other four sources — that measurement is this proposal's task 1.

2. **Keep LanceDB as a persistent on-disk index, or rebuild it in memory
   from Parquet at startup.** `STORAGE_EVIDENCE.md` §6.3 measured hybrid
   search (Tantivy + LanceDB via RRF) at 200–270 ms against Tantivy-only's
   3–7 ms, with the cost dominated by query embedding rather than LanceDB's
   own read, and found hybrid returns results Tantivy's lexical match
   misses on every query tested — most starkly for zero-lexical-match terms
   like "standup," where Tantivy found nothing and the vector leg still
   returned 10 relevant hits. The quality gain is real; keeping LanceDB is
   justified on that basis alone. What is open is whether an 8.8 MB
   persistent index is worth maintaining (crash recovery, upgrade
   migrations, `reindex`/`reprocess` write paths) versus rebuilding an
   in-memory vector structure from Parquet at daemon startup, given the
   corpus is currently small enough that either is plausible.
   Recommendation: **keep it persistent for now**; rebuild-at-startup adds
   daemon startup latency proportional to corpus size with no measured
   benefit at the current 1,147-record scale, and revisit only if corpus
   growth or `reindex` frequency makes the on-disk index a maintenance
   burden.

3. **Give `inbox.rs` and `vault_organize.rs` file-level pushdown for their
   Parquet reads, or leave full-corpus `read_all`.** §6.4 measured
   `read_all` across all 217 Parquet files at 17–100 ms — cheap today, but
   `O(total records)` for two callers that each need `O(matching records)`
   (a 2-day window for inbox processing; a single-source filter for the PARA
   audit), against write-side partitioning by `source/month` that already
   supports skipping files outside a date range. Recommendation: **defer**.
   The gap is real but currently masked by corpus size (1,147 records, 2.1
   MB); implement pushdown when either caller's measured cost crosses a
   threshold that matters (this proposal does not set one — that is a
   decision for whoever picks this back up when corpus size has actually
   grown), not preemptively.

None of the three recommendations above are implemented by this proposal.
They are the input the maintainer needs to accept, reject, or amend before
an implementing change (with its own tasks and tests) is written.

## Capabilities

### New Capabilities
(none — decision-only proposal)

### Modified Capabilities
(none — decision-only proposal; a follow-up implementing change will modify
`ingestion-pipeline` and/or `search-filtering` depending on which
recommendations above are accepted)

## Impact

- No code changes. Affected once accepted: `crates/backend/src/fetch_cycle.rs`
  (source list), `crates/kb-storage/src/vector_store.rs` (persistence
  strategy), `crates/backend/src/inbox.rs` and
  `crates/backend/src/vault_organize.rs` (`read_all` call sites), pending
  which recommendations are accepted.
- Reference: `openspec/changes/remove-accreted-architecture/STORAGE_EVIDENCE.md`
  (tasks 6.1–6.5), the evidence this recommendation is built on.
