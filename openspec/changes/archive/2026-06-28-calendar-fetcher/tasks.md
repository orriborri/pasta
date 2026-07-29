## Implementation

- [x] Create `crates/kb-fetchers/src/calendar.rs` with `CalendarFetcher` struct
- [x] Parse `gog calendar events --json` output into `Vec<Record>`
- [x] Handle both `dateTime` and `date` formats for all-day events
- [x] Filter out cancelled events
- [x] Add `pub mod calendar` to `crates/kb-fetchers/src/lib.rs`
- [x] Add `"calendar"` source to CLI sync match in `crates/kb-cli/src/main.rs`
- [x] Test: `kb sync --source calendar` fetches and indexes events
