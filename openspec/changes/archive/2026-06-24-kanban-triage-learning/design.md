## Context

The pasta inbox-processor agent creates task files from feeds. Task List Kanban shows tasks in columns based on tags. Currently the agent assigns tags — the user wants to own that decision. Over time, the system should learn from their choices and suggest columns for new items.

## Goals / Non-Goals

**Goals:**
- New tasks arrive untagged → Uncategorized column on Kanban
- User drags to assign column → tag written to file by Task List Kanban plugin
- Pasta detects tag changes and records pattern (source type + keywords → column)
- Daily note shows suggestions for untagged items after enough patterns exist (≥3 consistent decisions)

**Non-Goals:**
- Auto-tagging without user confirmation (explicitly rejected)
- ML/embeddings for pattern matching (keep it simple: source + keyword rules)
- Real-time suggestions (daily note is sufficient)

## Decisions

### 1. Pattern storage: JSON file in vault

**Decision**: Store patterns in `0. Inbox/triage-patterns.json` — a simple JSON array of rules.

```json
[
  { "source": "gitlab", "keywords": ["review", "MR"], "column": "today", "hits": 5, "misses": 0 },
  { "source": "gmail", "keywords": ["budget", "alert"], "column": "this-week", "hits": 3, "misses": 1 },
  { "source": "linear", "keywords": ["urgent"], "column": "today", "hits": 4, "misses": 0 }
]
```

**Rationale**: Simple, human-readable, editable. User can tweak rules manually. No database needed.

### 2. Pattern detection: vault-maintenance scans Tasks/ for tag changes

**Decision**: During each vault-maintenance cycle, scan `Tasks/` files for:
- Files that have a column tag (`#today`, `#this-week`, `#later`, `#backlog`) AND a `source:` field
- Compare against the patterns file
- If the source+keywords match an existing rule, increment `hits`
- If source matches but column differs, increment `misses`
- If no rule exists and we've seen this pattern 3+ times with same column, create a rule

**Rationale**: Runs daily, no real-time overhead. Task List Kanban already writes the tag when user drags — we just read the result.

### 3. Suggestion threshold: 3 hits, <30% miss rate

A pattern suggests a column when `hits >= 3` AND `misses / (hits + misses) < 0.3`.

### 4. Daily note triage section

The daily-writer agent reads untagged tasks and the patterns file, then writes:

```markdown
## 📥 Triage (4 items)
- [[Task-name]] → 💡 #today (MR review pattern, 5/5 confidence)
- [[Task-name-2]] → 💡 #this-week (budget alert pattern, 3/4)
- [[Task-name-3]] — no suggestion
- [[Task-name-4]] — no suggestion
```

User ignores this or uses it as a reference when dragging on the board.

## Risks / Trade-offs

- [Pattern staleness] → Old patterns may not reflect current priorities. → Mitigation: decay hits by 1 monthly, remove rules with 0 hits.
- [Keyword extraction is naive] → Just splits task title into words. Won't catch semantic similarity. → Acceptable for v1; upgrade later if needed.
