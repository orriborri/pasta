---
name: weekly-review
description: Generate multi-project weekly retro from git history, daily notes, ops activity, and current state. Consolidates work across infrastructure, product, personal, and ops into structured format. Organizes daily folder by month + week.
---

# Weekly Review

Automated workflow to compile a comprehensive weekly retrospective spanning multiple project areas: infrastructure, product, ops, and personal projects. Runs post-week to document Done/Next/Blockers per project.

## Trigger

Use this when:
- End of week (~Friday or Monday looking back)
- Need to consolidate daily notes → weekly summary across multiple projects
- Cleaning up daily folder structure
- Building a retro for external communication or team sync
- Multiple concurrent projects need status roll-up

Invoke: `weekly-review` or `/weekly-review`

## Workflow

### 1. Extract Work Done (Multi-Project)

#### Option A: Per-Repo Git Scan
For each project repo:
```bash
cd /path/to/project-repo
git log --oneline --all --author="$user" --since="$week_start" --until="$week_end"
```

**Projects to scan:**
- Personal: pasta, llm-wiki
- Infrastructure: cdk, eks-workloads, cloudformation
- Product: mononode, nativeflow, bidding-stream
- Shared: ad-events, gitlab-components

#### Option B: Daily Notes Activity Feed
Parse daily notes for project mentions:
```bash
grep -h "mononode\|cdk\|nativeflow\|eks-\|cloudformation\|bidding-stream" /path/to/Daily/*.md | \
  grep -E "MR|PR|feat|fix|OPS-|AD-|WEB-|NATIVEFLOW|PLATFORM" | sort | uniq
```

**Extract:**
- Features/fixes in review (MR numbers)
- Infrastructure changes (feat/fix commits)
- Product work (feature flags, observability, dependencies)
- Ops incidents (RDS alarms, DNS migration, cost anomalies)

#### Option C: Pasta KB (When Connected)
Query Pasta for:
- Cross-project activity feed
- Recent decisions per project area
- Incident/alert log

**For now** (kb-mcp pending): Use daily notes + git scan

### 2. Aggregate Daily Notes

Source: `/path/to/Daily/` folder

- Read daily capture files (if populated)
- Extract key events, blockers, decisions
- Link to git commits where relevant

### 3. Build Multi-Project Retro Template

Use structure from recent weekly files (e.g., `weekly-oscar-2026-09-28.md`):

```markdown
# Oscar
**Weekly Readpeak Report — Week ending YYYY-MM-DD**

## [Project] / [Area] — [Subarea]

### Done
- **[Task/Feature]** (linked MR !123, OPS-456)
  - Details, resolution, impact
  - Related links: [[Issue]], [[Design Doc]]

### In Progress / Next
- Priority item 1 (MR link)
- Priority item 2 (Issue link)

### Blockers / Decisions Needed
- [Risk/decision with context]

---

## Cross-Project Status

| Project | Status | Key Item | Blocker |
|---------|--------|----------|---------|
| Personal | ✓ Done | LLM toolkit merged | kb-mcp reconnect |
| Infrastructure | In flight | IAM agent roles | CfnEventBusPolicy deprecation |
| Product | In flight | OPS-329 observability | Pipeline test failures |
| Ops | Active | RDS alarm response | Investigation ongoing |
```

**Per-project sections:**
1. Personal / Pasta
2. Infrastructure (cdk, eks-workloads, cloudformation)
3. Product (mononode, nativeflow, bidding-stream)
4. Ops / On-Call
5. Specs / Planning
6. Cross-Cutting / Decisions Needed

### 4. Consolidate Daily → Weekly Summary

Create: `Daily/Weekly/Week-YYYY-MM-DD-to-YYYY-MM-DD.md`

```markdown
# Week of [DATE RANGE]

## 📊 Summary
[1-2 sentence overview]

## ✅ Accomplishments This Week
- [Grouped by area]

## 🎯 Next Week Priorities
1. [Ranked]

## 🚧 Current Blockers

## 📈 Metrics
- Commits, tests added, specs drafted

## Daily Notes Archive
- **Sept 22**: [1-liner]
- **Sept 23**: [1-liner]
```

### 5. Organize Daily Folder

Structure:
```
Daily/
├── 2026-09/              (current month dailies)
├── Weekly/               (weekly summaries)
│   └── Week-YYYY-MM-DD-to-YYYY-MM-DD.md
└── .archive/
    ├── 2026-05/ to 2026-08/  (old months)
    └── [templates, empty files]
```

**Commands:**
```bash
# Archive old months
mkdir -p .archive
for dir in 2026-06 2026-07 2026-08; do
  [ -d "$dir" ] && mv "$dir" ".archive/$dir"
done

# Organize current month
mkdir -p 2026-09 Weekly
for f in YYYY-MM-*.md; do mv "$f" "2026-09/$f"; done

# Move weekly summaries
for f in Week-*.md; do mv "$f" "Weekly/$f"; done

# Archive empty/template files
mv Today.md .archive/ 2>/dev/null
```

## Files Generated

1. **Detailed retro**
   - Location: `2. Areas/Infra Updates/weekly-oscar-YYYY-MM-DD.md`
   - Audience: Personal + team (if shared)
   - Contains: Full project breakdown, detailed next steps, blockers

2. **Weekly summary**
   - Location: `0. Inbox/Daily/Weekly/Week-YYYY-MM-DD-to-YYYY-MM-DD.md`
   - Audience: Personal reference
   - Contains: Condensed accomplishments, priorities, metrics

## Integration with Personal Agent

The weekly review feeds into `personal-agent`:
- Prior retros stored in `2. Areas/Infra Updates/`
- Weekly summaries available for KB search
- Used as evidence in multi-week planning

Link from weekly file:
```markdown
See detailed retro: [[weekly-oscar-2026-09-28]]
See KB guide: [[Weekly-Review-Process]]
```

## Related Skills

- [[personal-agent]] — Cross-session context & knowledge system
- [[raw-inbox-memory]] — Capture durable personal facts

## Automation Hooks

When automating (future):
```bash
# Hook on Friday evening or Monday morning
claude /weekly-review --date "2026-09-28" --author "Oscar"
```

Would:
1. Extract git commits since Monday
2. Read daily notes
3. Generate both retro files
4. Reorganize Daily folder
5. Return summary

---

**Template files:**
- Recent retro: `2. Areas/Infra Updates/weekly-oscar-2026-09-13.md` (example structure)
- Recent summary: `0. Inbox/Daily/Weekly/Week-2026-09-22-to-09-28.md` (example)
