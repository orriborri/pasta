## Phase 1: Feedback Storage

- [ ] Add `feedback` table to SyncState SQLite (query, record_id, judgment, created_at)
- [ ] Add methods: `add_feedback`, `get_feedback_for_query`, `clear_feedback`
- [ ] Store last query in `meta` table when search runs

## Phase 2: Interactive Search TUI

- [ ] Add `crossterm` dependency for terminal raw mode / key events
- [ ] After search results are returned, enter interactive mode (skip if stdout is not a TTY)
- [ ] Display results with navigation (j/k or ↑/↓)
- [ ] Highlight current result
- [ ] `g` key marks current result as good, shows ✓
- [ ] `b` key marks current result as bad, shows ✗
- [ ] `q` or Enter exits feedback mode
- [ ] Save feedback to SQLite on each keypress

## Phase 3: Apply Feedback to Ranking

- [ ] Load feedback for current query in hybrid_search
- [ ] Apply boost/penalty multipliers to RRF scores (1.2x good, 0.7x bad, capped at 3x/0.3x)
- [ ] Results with prior "bad" for this query drop in ranking or get filtered

## Phase 4: MCP API

- [ ] Include `record_id` in `search_knowledge` MCP tool response
- [ ] Add `provide_feedback` MCP tool (params: query, record_id, judgment)
- [ ] Wire to same SQLite feedback table

## Phase 5: Stats

- [ ] Add `kb stats` command (record count, feedback count, top penalized records)
