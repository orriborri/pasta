## 1. Decide whether to index `vault` in the automated fetch cycle

- [ ] 1.1 Measure the cost of adding `vault` to `fetch_cycle.rs`'s source list on a copy of the real vault: wall-clock time per cycle, and how much of the 225+ vault record set changes per day of normal editing
- [ ] 1.2 Decide a cadence: every fetch cycle, or a separate slower schedule, given vault content changes on note edits rather than API polling
- [ ] 1.3 Record the decision and, if "yes," open its own implementing change (source list edit, cadence, and a test proving `audit_para`/`route_new_tasks_semantic` see fresh vault content after a cycle)

## 2. Decide LanceDB's persistence strategy

- [ ] 2.1 Re-run `STORAGE_EVIDENCE.md`'s §6.3 latency/quality comparison if corpus size has changed materially since 2026-08-10
- [ ] 2.2 If the on-disk index is kept: no action needed beyond this record
- [ ] 2.3 If rebuild-at-startup is chosen instead: open its own implementing change with a measured daemon-startup latency budget

## 3. Decide Parquet read pushdown

- [ ] 3.1 Re-measure `read_all` cost in `inbox.rs` and `vault_organize.rs` against the corpus size at decision time
- [ ] 3.2 If cost has crossed a threshold that matters for daemon responsiveness: open its own implementing change adding file-level date/source pushdown, using the existing `source/month` write partitioning
- [ ] 3.3 If not: leave as-is and note the corpus size at which this was last checked, so the next reviewer has a baseline
