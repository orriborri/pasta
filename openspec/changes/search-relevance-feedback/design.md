## Context

kb-engine has hybrid search (vector + full-text + RRF merge) but no way to learn from user behavior. Results quality depends entirely on embedding quality and TF-IDF scoring. Users can't signal relevance.

## Goals / Non-Goals

**Goals:**
- Let users mark search results as good/bad after seeing them
- Store feedback persistently
- Apply learned boosts/penalties to future searches
- Keep it simple — no ML model training, just score adjustments

**Non-Goals:**
- Click-through tracking (CLI doesn't have implicit signals)
- Personalization per-user (single user system)
- Active learning / query suggestion
- Retraining embeddings based on feedback

## Decisions

### 1. Feedback stored in SQLite `feedback` table

```sql
CREATE TABLE feedback (
    id INTEGER PRIMARY KEY,
    query TEXT NOT NULL,
    record_id TEXT NOT NULL,
    judgment TEXT NOT NULL CHECK (judgment IN ('good', 'bad')),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_feedback_record ON feedback(record_id);
```

### 2. Score adjustment via simple boost factor

After RRF merge, apply a boost based on historical feedback for each record:
- Records with `good` judgments for similar queries → boost score by 1.2x per positive
- Records with `bad` judgments → penalize by 0.7x per negative
- Cap at 3x boost / 0.3x floor to prevent runaway

"Similar queries" = same query string (exact match for now). Future: semantic similarity between queries.

### 3. Interactive TUI for feedback

After search, enter an interactive mode where results are displayed and the user can navigate and judge:

```
$ kb search "double verify"

─── 1 (score: 0.0164) ───
source: slack | 2026-06-10 | #devops - Oscar
added an integration to this channel: DeployBot

  [g] good  [b] bad  [↑↓] navigate  [q] quit

─── 2 (score: 0.0161) ───
source: slack | 2026-06-05 | #developmentteam - tuomo
...
```

Controls:
- `g` — mark current result as good (relevant)
- `b` — mark current result as bad (irrelevant)
- `↑`/`↓` or `j`/`k` — navigate between results
- `q` or `Enter` — exit feedback mode

Feedback is saved immediately on keypress. Non-interactive mode (piped output) skips the TUI and just prints results as before.

### 4. MCP feedback tool

The MCP server exposes a `provide_feedback` tool so AI agents can programmatically mark results:

```json
{
  "name": "provide_feedback",
  "params": {
    "query": "double verify",
    "record_id": "slack-C04AB-1234567.000",
    "judgment": "bad"
  }
}
```

The `search_knowledge` tool response includes `record_id` in each result so the agent can reference it. Flow:
1. Agent calls `search_knowledge` → gets results with IDs
2. Agent determines relevance based on user context
3. Agent calls `provide_feedback` for irrelevant/relevant results

### 5. Stats command

```
$ kb stats
Records: 4,400 | Feedback: 23 (15 good, 8 bad)
Top penalized: slack-intercom (4 bad)
```

## Risks / Trade-offs

- [Exact query match is crude] → Good enough for single user. Expand later to semantic query similarity.
- [Feedback can bias toward familiar results] → Cap boost/penalty. User can reset with `kb feedback reset`.
- [Extra step for user] → Friction is intentional. Only mark results when they're clearly wrong.
