## 1. Backend Connection Robustness

- [x] 1.1 Add `connect_or_start_backend` function that tries connecting before starting
- [x] 1.2 Detect stale sockets (exists but connection refused) and clean up
- [x] 1.3 Find orphaned `pasta-backend` processes via `pgrep`
- [x] 1.4 Prompt user to kill orphaned backends or abort
- [x] 1.5 Wait for socket with connect retry after starting backend

## 2. Deferred Good-Morning Agent

- [x] 2.1 Add `fetch_completed` flag to App state
- [x] 2.2 Set flag on `FetchComplete` event from backend
- [x] 2.3 Replace fixed 5s timer with event-driven trigger
- [x] 2.4 Show "Waiting for fetchers..." in status bar while pending

## 3. Structured Today View

- [x] 3.1 Parse daily note into sections (meetings, tasks, waiting)
- [x] 3.2 Extract priority (P1/P2/P3) and clean title from task lines
- [x] 3.3 Render meetings panel with yellow styling
- [x] 3.4 Render tasks list with priority coloring (P1 red, P2 yellow, P3 gray)
- [x] 3.5 Render waiting-for panel
- [x] 3.6 Highlight selected task with cursor indicator

## 4. Task Completion from TUI

- [x] 4.1 Add `daily_task_count` method for Today tab navigation
- [x] 4.2 Enable j/k selection in Today tab
- [x] 4.3 Add `x` keybind to toggle task checkbox
- [x] 4.4 Write toggled state back to daily note file

## 5. Deduplicate API Calls

- [x] 5.1 Skip history sync on same tick as native fetchers to avoid duplicate API calls

## 6. Unified Feed→History Pipeline

- [x] 6.1 Refactor native fetchers to return raw API data (not just write summaries)
- [x] 6.2 Feed writer consumes raw data → writes `.feeds/` summaries (existing behavior)
- [x] 6.3 History writer consumes same raw data → writes `.history/` records
- [x] 6.4 Index new history records into LanceDB after processing
- [x] 6.5 Run vault organizer after indexing completes

## 7. Manual-Only Backfill

- [x] 7.1 Remove automatic history sync from the scheduler loop
- [x] 7.2 Add `Command::Backfill` IPC command for manual trigger
- [x] 7.3 Add TUI keybind or chat command to trigger backfill manually
- [x] 7.4 Backfill fetches older data beyond what native fetchers cover
