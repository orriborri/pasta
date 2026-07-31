## ADDED Requirements

### Requirement: Fetch cycle is single-flight
The system SHALL run at most one fetch/index cycle at a time, whether triggered by the scheduler or manually, so concurrent writers cannot touch the same Parquet, Tantivy, LanceDB, and SQLite stores.

#### Scenario: Manual trigger during a scheduled cycle
- **WHEN** a scheduled fetch cycle is running and the user presses the force-fetch key
- **THEN** no second cycle starts, and the user is told a cycle is already running

#### Scenario: Repeated manual triggers
- **WHEN** the user triggers a fetch twice in quick succession
- **THEN** the second trigger is rejected until the first completes

#### Scenario: Running cycle is visible to the client
- **WHEN** a manually triggered cycle is in progress
- **THEN** the client shows it as running, so the user is not led to trigger it again

### Requirement: Completed-fetcher set is merged, not replaced
The system SHALL merge fetch-cycle results into the completed-fetcher set rather than overwriting it wholesale, so updates recorded during the cycle are preserved.

#### Scenario: Agent finishes while a fetch cycle is running
- **WHEN** an agent completes and is recorded in the completed set during a fetch cycle
- **THEN** that record survives the cycle's own update to the set

## MODIFIED Requirements

### Requirement: Fetch cycle reports success conditionally
The system SHALL mark every fetcher in the cycle as completed when the cycle finishes, regardless of whether an individual source returned records. Completion tracks the cycle having run, not per-source yield, because sources legitimately return nothing and some arrive via another source's feed.

#### Scenario: A source returns no records
- **WHEN** a source returns zero records during an otherwise successful cycle
- **THEN** its fetcher name is still inserted into the completed set, and dependent agents trigger

#### Scenario: The cycle fails before completing
- **WHEN** fetching fails or times out and the cycle aborts
- **THEN** no fetcher names are inserted, and dependent agents do not trigger

## REMOVED Requirements

### Requirement: Atomic state file writes
**Reason**: `_sync_state.json` no longer exists. Sync state moved to SQLite (`state.db`) in the kb-engine migration, which provides its own write serialization.
**Migration**: None required. State is written through `kb_core::sync_state`.

### Requirement: Indexer does not overwrite its own hash updates
**Reason**: `index_new_files`, `run_sync`, and `sync_once` were deleted in the history-to-kb migration. Hash persistence now lives in `kb-sync` behind a single write path.
**Migration**: None required.

### Requirement: Parallel sync state merge preserves concurrent updates
**Reason**: The parallel per-source sync tasks and the shared state clone they contended over no longer exist. The equivalent concern for the daemon's in-memory completed set is now covered by "Completed-fetcher set is merged, not replaced".
**Migration**: None required.

### Requirement: Linear history sync uses configured binary path
**Reason**: `history/sync_linear.rs` was deleted. Linear ingestion lives in `kb-fetchers` and resolves its binary through the shared resolver.
**Migration**: None required.
