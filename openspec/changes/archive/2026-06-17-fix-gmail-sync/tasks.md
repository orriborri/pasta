## 1. State Schema

- [x] 1.1 Add `newest_fetched: Option<String>` and `oldest_fetched: Option<String>` fields to `SourceState`
- [x] 1.2 Ensure serde default handles missing fields for backward compat with existing state files

## 2. run_cmd Timeout

- [x] 2.1 Refactor `run_cmd` to spawn child process with 60s timeout using `wait_timeout`
- [x] 2.2 Capture and log stderr via tracing on non-zero exit or timeout
- [x] 2.3 Return `None` on timeout after killing child process

## 3. Gmail Incremental Fetch

- [x] 3.1 On first run (no boundaries): query `after:{now-7d}`, set both boundaries
- [x] 3.2 On subsequent run: forward query `after:{newest_fetched}`, update `newest_fetched`
- [x] 3.3 On subsequent run: backward query `before:{oldest_fetched} after:{oldest_fetched-7d}`, update `oldest_fetched`
- [x] 3.4 Remove hardcoded `newer_than:6m` fallback

## 4. Slack Bounded Fallback

- [x] 4.1 Replace `oldest_ts(DM_DAYS)` / `oldest_ts(CHANNEL_DAYS)` fallback with `oldest_fetched` from state
- [x] 4.2 On first run (no boundaries): set `oldest_fetched = now - 7d`, `newest_fetched = now`
- [x] 4.3 On subsequent run: backward query uses `oldest_fetched - 7d` for channels without cursor
- [x] 4.4 Remove `CHANNEL_DAYS` / `DM_DAYS` constants

## 5. Linear Incremental Fetch

- [x] 5.1 On first run: query issues updated in last 7 days, set boundaries
- [x] 5.2 On subsequent run: forward query `updatedAt.gte:{newest_fetched}`
- [x] 5.3 On subsequent run: backward query `updatedAt.gte:{oldest-7d} updatedAt.lt:{oldest}`
- [x] 5.4 Update boundaries after each successful fetch
