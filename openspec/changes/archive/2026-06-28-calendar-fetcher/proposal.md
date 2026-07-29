## Why

The kb-engine syncs Slack, Gmail, Linear, Git, and vault files — but not Calendar. Calendar events contain meeting context (who, when, agenda, decisions) that's highly relevant when searching for "when did we discuss X" or "what was the meeting about Y". Without it, kb-engine has a blind spot for time-based work coordination.

The `gog` CLI already supports `gog calendar events --json --from --to --all` with full event data (attendees, description, links). No new OAuth or API work needed.

## What Changes

- New fetcher: `kb-fetchers/src/calendar.rs`
- Fetches events via `gog calendar events --json --from <cursor> --all`
- Converts events to `Vec<Record>` with attendees as participants, description as content
- Incremental: tracks last-fetched timestamp via SyncState cursor

## Capabilities

### Modified Capabilities
- `kb-api`: `kb sync` now includes `calendar` as a source; `kb sync --source calendar` works standalone

## Impact

- New file: `crates/kb-fetchers/src/calendar.rs` (~80 LoC)
- Modified: `crates/kb-fetchers/src/lib.rs` (add `pub mod calendar`)
- Modified: `crates/kb-cli/src/main.rs` (add `"calendar"` to source match)
- Data: new Parquet files at `~/.kb/raw/calendar/YYYY-MM/{ts}.parquet`
