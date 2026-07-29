## 1. Dependencies and Setup

- [x] 1.1 Add `indicatif` crate to workspace and backend Cargo.toml
- [x] 1.2 Create `SyncResult` struct returned by each syncer

## 2. Refactor Tracing

- [x] 2.1 Remove stderr layer from tracing subscriber (keep file-only)
- [x] 2.2 Ensure `./logs/tracing.log` still captures all events with timestamps

## 3. Progress Display

- [x] 3.1 Create progress display module with `MultiProgress` setup showing all stages as pending on startup
- [x] 3.2 Add helper to transition stage status (pending → active → done) with appropriate indicators (○ ▸ ✓)
- [x] 3.3 Show memory usage on the active stage line, updated periodically
- [x] 3.4 Show item count summary on completed stage lines

## 4. Integrate Syncers

- [x] 4.1 Update `run_sync` orchestrator to drive the progress display, updating each stage before/after calling syncers
- [x] 4.2 Update slack syncer to return `SyncResult` with channel count and message count
- [x] 4.3 Update gmail syncer to return `SyncResult` with thread count
- [x] 4.4 Update linear syncer to return `SyncResult` with issue count
- [x] 4.5 Update wiki syncer to return `SyncResult` with file count
- [x] 4.6 Update repos syncer to return `SyncResult` with file count

## 5. Indexer Progress Bar

- [x] 5.1 Add progress bar with percentage and file count (done/total) for the indexing stage
- [x] 5.2 Emit ETA based on elapsed time and files remaining
- [x] 5.3 Update progress bar from embedder task as files are processed

## 6. Final Summary

- [x] 6.1 Track peak memory across the entire sync run
- [x] 6.2 Display "Done in Xm Ys · Peak memory: NMB" after all stages complete
- [x] 6.3 Display errors inline on failed stage lines without stopping the overall progress
