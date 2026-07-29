## Context

Roadmap initiatives have target end dates (from `Roadmap/*.md` frontmatter). Tasks under each initiative (`Tasks/<initiative>/`) have completion status. By comparing progress ratio vs time ratio we can flag risks.

## Goals / Non-Goals

**Goals:**
- Compute health status per initiative daily
- Surface it in the daily note as a simple table
- Three states: on track, at risk, off track

**Non-Goals:**
- Predictive scheduling (adjusting dates)
- Notifications/alerts (daily note is sufficient)
- Gantt modifications

## Decisions

### 1. Health formula

```
time_elapsed = (today - start) / (end - start)
completion   = done_tasks / total_tasks

if total_tasks == 0: 🟢 (no tasks yet, nothing to track)
if completion >= time_elapsed: 🟢 On track
if completion >= time_elapsed - 0.2: 🟡 At risk
else: 🔴 Off track
```

Special cases:
- Initiative hasn't started yet (today < start): 🟢 unless tasks exist and none done
- Initiative past due: 🔴 if not 100% complete

### 2. Data sources

- **Target dates**: `start:` and `end:` from `Roadmap/<initiative>.md` frontmatter
- **Task completion**: Count files in `Tasks/<initiative>/` — done = has `status: done` or tag `#done` in frontmatter, total = all `.md` files

### 3. Output in daily note

The daily-writer reads the output of `compute_roadmap_health()` (exposed via the MCP `get_daily` tool or directly by the agent reading files) and formats the table.

## Risks / Trade-offs

- [Inaccurate if tasks aren't granular] → A single huge task shows 0% until done. Mitigation: encourage breaking into subtasks.
- [Start/end dates missing] → If a Roadmap file has no dates, skip it in the health table.
