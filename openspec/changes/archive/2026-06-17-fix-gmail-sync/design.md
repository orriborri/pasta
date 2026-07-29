## Context

All sync sources (Gmail, Slack, Linear) risk re-fetching data. Gmail hangs on first run trying 6 months. Slack falls back to 90/180 days for channels without cursors. We need a uniform window-based pattern.

## Goals / Non-Goals

**Goals:**
- First sync of any source completes quickly (7 days window)
- Each subsequent sync extends coverage forward and backward without overlap
- State tracks exact fetch boundaries per source so nothing is ever re-fetched
- External commands have a timeout to prevent hangs

**Non-Goals:**
- Fetching all history in one run
- Deduplication within already-fetched ranges
- Changing the Slack per-channel cursor mechanism (keep it, but bound the fallback)

## Decisions

### 1. Uniform state per source

```rust
pub struct SourceState {
    pub last_sync: Option<DateTime<Local>>,
    pub newest_fetched: Option<String>,  // "YYYY/MM/DD" or unix ts
    pub oldest_fetched: Option<String>,  // "YYYY/MM/DD" or unix ts
    pub cursors: HashMap<String, String>,
}
```

### 2. Fetch logic per sync run

```
if no state:
    fetch(now - 7d, now)
    set newest_fetched = now, oldest_fetched = now - 7d
else:
    forward: fetch(newest_fetched, now)  → update newest_fetched = now
    backward: fetch(oldest_fetched - 7d, oldest_fetched) → update oldest_fetched -= 7d
```

### 3. Source-specific mapping

| Source | Forward query | Backward query |
|--------|--------------|----------------|
| Gmail | `after:{newest}` | `before:{oldest} after:{oldest-7d}` |
| Slack | Per-channel cursor (existing) with fallback = `oldest_fetched` not 90d | `oldest={oldest-7d}` for channels without cursor |
| Linear | `updatedAt.gte:{newest}` | `updatedAt.gte:{oldest-7d} updatedAt.lt:{oldest}` |

### 4. Slack keeps channel cursors

Slack already has per-channel `ts` cursors that work well. The change is only to cap the **fallback** (when no cursor exists) to `oldest_fetched` instead of a hardcoded 90/180 days.

### 5. Timeout on run_cmd (60s)

Spawn child, kill if exceeds 60 seconds, log stderr.

## Risks / Trade-offs

- **[Trade-off] Slow backfill** — takes N syncs to cover N weeks of history. Acceptable for background.
- **[Risk] Slack cursor already handles forward** → Keep existing cursor logic, just bound the fallback.
- **[Risk] Linear API pagination** → first 7 days may still be large if many issues. Limit to 50 per page (already done).
