## 1. Feed formatting from Records

- [x] 1.1 Add feed format functions to `kb-sync` crate: `format_slack_feed`, `format_gmail_feed`, `format_linear_feed`, `format_gitlab_feed`, `format_calendar_feed` — each takes `&[Record]` and returns markdown matching current `.feeds/` format
- [x] 1.2 Add `write_feeds(records: &[Record])` function that groups records by source and writes `.feeds/{source}.md`

## 2. Unify fetch cycle

- [x] 2.1 Update `kb-sync::run()` to return records before indexing, split into `fetch() -> Vec<Record>` and `index(records)` so feeds can be written between them
- [x] 2.2 Rewrite `fetch_cycle.rs` to: call `kb_sync::fetch()` → write feeds → update completed set → call `kb_sync::index()`
- [x] 2.3 Update completed-set logic to derive from record sources (non-empty slack records → `"slack-fetcher"`, etc.)

## 3. Remove backend fetchers

- [x] 3.1 Delete `crates/backend/src/fetchers/{slack,gmail,linear,gitlab,calendar}.rs`
- [x] 3.2 Remove their `mod` declarations and `CommandRunner` trait from `fetchers/mod.rs`
- [x] 3.3 Keep `fetchers/mod.rs` with `write_feed` helper and `vault_manager.rs`
- [x] 3.4 Remove unused imports and dependencies from backend `Cargo.toml`

## 4. Remove kb-sync schedule

- [x] 4.1 Remove `kb-sync` from `KNOWN_NATIVE_SCHEDULES` in `crates/common/src/data.rs`
- [x] 4.2 Remove `kb-sync` default entry from config defaults in `crates/common/src/config.rs`
- [x] 4.3 Remove `run_kb_sync` function and its REGISTRY entry from `scheduler.rs`
- [x] 4.4 Add deprecation log if `[schedules.kb-sync]` is present in user config

## 5. Verify and clean up

- [x] 5.1 `cargo check` — verify full workspace compiles
- [x] 5.2 Run `kb sync` CLI manually to confirm it still works via the shared crate
- [ ] 5.3 Start backend, verify TUI schedule tab shows fetch cycle running without kb-sync row — not re-exercised during 2026-08-05 reconciliation
- [ ] 5.4 Verify `.feeds/` files are populated after a fetch cycle completes — not re-exercised during 2026-08-05 reconciliation
