## Why

The sync process outputs verbose tracing logs to stderr that are hard to read during normal use. Users can't tell what's happening, what's pending, or how long it will take. When something fails (e.g., OOM), there's no clear indication of where it stopped. The CLI needs a clean, human-friendly progress display separate from debug logs.

## What Changes

- Add a real-time CLI progress display showing all sync stages with status (pending/active/done)
- Show memory usage, item counts, and ETA during the indexer pipeline
- Route verbose tracing exclusively to `./logs/tracing.log` (not stderr)
- Display a progress bar for the indexing stage
- Show a final summary with duration and peak memory

## Capabilities

### New Capabilities
- `cli-progress-display`: Real-time terminal progress UI for sync operations showing pending (○), active (▸), and completed (✓) stages with memory, counts, ETA, and progress bar

### Modified Capabilities

## Impact

- `crates/backend/src/main.rs` — replace stderr tracing layer with file-only; add progress rendering
- `crates/backend/src/history/mod.rs` — syncers return result summaries to drive the display
- `crates/backend/src/history/indexer.rs` — emit progress updates for the display
- New dependency: `indicatif` crate for progress bar and in-place terminal updates
- Tracing subscriber config changes: remove stderr writer, keep file writer only
