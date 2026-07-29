## Why

Search results often include irrelevant matches — especially for short or ambiguous queries. Without feedback, the system can't learn which results are useful. Users currently have no way to signal "this result is wrong" or "this is exactly what I needed", so result quality stagnates.

Adding explicit relevance feedback lets us:
- Track which results are useful vs noise for given queries
- Use feedback to re-rank future results (learned boosting)
- Identify patterns in bad results (e.g., always filtering bot messages, certain channels)
- Measure search quality over time

## What Changes

- Add a feedback mechanism to `kb search` — users can mark results as relevant/irrelevant
- Store feedback in SQLite (query, record_id, judgment)
- Use accumulated feedback to boost/penalize records in future searches
- Surface feedback stats via `kb stats`

## Capabilities

### New Capabilities
- `relevance-feedback`: Collect and apply user feedback on search results to improve ranking over time

### Modified Capabilities
- `kb-api`: Search results gain an interactive feedback mode in CLI; MCP returns result IDs for programmatic feedback

## Impact

- New SQLite table `feedback` in state.db
- Modified `kb search` CLI output (adds result IDs for reference)
- Modified hybrid search scoring (applies learned boosts)
- New command: `kb feedback <result-id> <good|bad>`
- New command: `kb stats` showing feedback summary
