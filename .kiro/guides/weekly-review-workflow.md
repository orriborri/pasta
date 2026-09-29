# Weekly Review Workflow

Cross-session multi-project knowledge system workflow for compiling comprehensive weekly retros spanning infrastructure, product, ops, and personal projects. Consolidates work from git history + daily notes + ops activity, maintaining persistent records.

## Overview

Each week, consolidate engineering work across multiple projects into:
1. **Detailed weekly retro** — per-project breakdown (Infrastructure, Product, Ops, Personal), next steps, blockers
2. **Weekly summary** — consolidated accomplishments, cross-project priorities, metrics, alerts
3. **Organized daily folder** — structured by month + week

**Scope:** Personal projects (pasta) + Readpeak infrastructure (cdk, eks-workloads, cloudformation) + Product (mononode, nativeflow, bidding-stream) + Ops (incidents, alerts, migrations)

## Quick Start

```bash
# Invoke the skill
/weekly-review

# Or manually follow the workflow
# 1. Extract git commits since week start
# 2. Read daily notes from Daily/ folder
# 3. Generate retro files using template
# 4. Reorganize Daily/ folder structure
```

## File Locations

### Inputs
- **Git history**: `git log --since=$week_start --until=$week_end`
- **Daily notes**: `/Obsidian/Readpeak/0. Inbox/Daily/`
- **Templates**: 
  - Detailed retro: `/Obsidian/Readpeak/2. Areas/Infra Updates/weekly-oscar-*.md` (examples)
  - Summary: `/Obsidian/Readpeak/0. Inbox/Daily/Weekly/Week-*.md` (examples)

### Outputs
- **Detailed retro**: `/Obsidian/Readpeak/2. Areas/Infra Updates/weekly-oscar-YYYY-MM-DD.md`
  - Organized by project/area (cdk, mononode, ops, etc.)
  - Done/Next/Blockers sections
  - Linked to git commits & issues

- **Weekly summary**: `/Obsidian/Readpeak/0. Inbox/Daily/Weekly/Week-YYYY-MM-DD-to-YYYY-MM-DD.md`
  - Consolidated accomplishments
  - Next week priorities
  - Metrics (commits, tests, specs)

- **Organized folder**:
  ```
  Daily/
  ├── 2026-09/              # Current month
  │   ├── 2026-09-22.md
  │   └── 2026-09-25.md
  ├── Weekly/               # Weekly summaries
  │   └── Week-2026-09-22-to-09-28.md
  └── .archive/
      ├── 2026-05/ through 2026-08/
      └── [templates, empty files]
  ```

## Process

### Phase 1: Extract Work from Multiple Projects (15 min)

**For each project repo:**
```bash
cd /path/to/project-repo
git log --oneline --all --author="$USER" --since="2026-09-22" --until="2026-09-29"
```

**Projects to scan:**
- **Personal**: `/home/orre/code/pasta`
- **Infrastructure**: cdk, eks-workloads, cloudformation (need paths/access)
- **Product**: mononode, nativeflow, bidding-stream (need paths/access)

**Or use daily notes activity feed:**
```bash
grep -h "MR\|PR\|feat\|fix\|OPS-\|AD-\|WEB-" /path/to/Daily/*.md | \
  grep -E "mononode|cdk|nativeflow|eks-|cloudformation" | sort | uniq
```

**For each project, capture:**
- Commit hash
- Linked issues/MRs (MR !123, OPS-456, AD-789)
- Feature/fix/spec/docs category
- Context (what problem it solves)
- Status (done, in review, in progress, blocked)

### Phase 2: Daily Notes Scan (5 min)

Source: `/Obsidian/Readpeak/0. Inbox/Daily/2026-09/`

For each day:
- Note key decisions, blockers, contexts
- Link to git commits if relevant
- Extract next-week priorities

### Phase 3: Build Multi-Project Retro (15 min)

**Detailed retro structure (organized per-project):**
```markdown
# Oscar
**Weekly Readpeak Report — Week ending YYYY-MM-DD**

## Personal / Pasta — LLM Wiki Toolkit

### Done
- **Merged PR #1** (MR !1, Sept 25)
  - Personal agent skills, raw inbox ingestion, tests, docs

### In Progress / Next
- MCP connection debug
- Weekly review automation

### Blockers
- kb-mcp session reconnection needed

---

## Infrastructure / CDK — IAM Agent Roles

### Done
- **AWS FinOps Agent** (payer account) — feat/security-agent-pentest-role
- **DevOps Agent monitoring** (staging + prod) — feat/devops-agent-monitoring-roles

### In Progress / Next
- Deploy to production

### Blockers
- Main branch validation gate recovery

---

## Product / Mononode — Observability

### Done
- **OPS-329: OpenTelemetry metrics** (MR !3281) — in review cycle

### In Progress / Next
- Resolve pipeline test failures
- Land MR

### Blockers
- Pipeline test failures

---

## Ops / On-Call

### Done
- RDS audit privilege alarm investigation initiated

### In Progress / Next
- Resolve RDS alarm (lateral escalation investigation)
- DNS migration decision (Route53)

### Blockers
- RDS privilege escalation requires prod remediation

---

## Cross-Project Status

| Area | Status | Key Item | Blocker |
|------|--------|----------|---------|
| Personal | ✓ Done | LLM toolkit merged | kb-mcp reconnect |
| Infrastructure | In flight | IAM roles staging-proven | Main branch CI issues |
| Product | In flight | OPS-329 observability | Pipeline test failures |
| Ops | Active | RDS alarm response | Investigation ongoing |
```

**Weekly summary structure (multi-project):**
```markdown
# Week of YYYY-MM-DD to YYYY-MM-DD

## 📊 Summary
[1-2 sentences: overall progress across all projects]

## ✅ Accomplishments This Week

### Personal System
- [Item 1]
- [Item 2]

### Infrastructure
- [Item 1]
- [Item 2]

### Product
- [Item 1]
- [Item 2]

### Ops / On-Call
- [Item 1]

## 🎯 Next Week Priorities
1. [Cross-project priority 1]
2. [Cross-project priority 2]
3. [Cross-project priority 3]

## 🚧 Current Blockers
- **[Project]**: [Blocker with impact]
- **[Project]**: [Blocker with impact]

## 📈 Metrics
- Commits (all projects): N
- MRs in review: N
- Production alerts: N
- Specs drafted: N

## Project Status
- **Personal**: Status
- **Infrastructure**: Status
- **Product**: Status
- **Ops**: Status

## Daily Notes Archive
- **Sept 22**: [1-liner]
- **Sept 23**: [1-liner]
- **Sept 24**: [1-liner]
- **Sept 25**: [1-liner]
```

### Phase 4: Organize Folder (5 min)

```bash
cd "/path/to/Daily"

# Create structure
mkdir -p .archive Weekly
mkdir -p 2026-09

# Move old months to archive
for dir in 2026-06 2026-07 2026-08; do
  [ -d "$dir" ] && mv "$dir" ".archive/$dir"
done

# Move Sept files into month subdir
for f in 2026-09-*.md; do
  [ -f "$f" ] && mv "$f" "2026-09/$f"
done

# Move weekly summaries
for f in Week-*.md; do
  [ -f "$f" ] && mv "$f" "Weekly/$f"
done

# Archive templates/empty files
mv Today.md .archive/ 2>/dev/null
```

## Templates

### Recent Examples
- **Detailed retro**: `weekly-oscar-2026-09-13.md` in `2. Areas/Infra Updates/`
- **Weekly summary**: `Week-2026-09-22-to-09-28.md` in `Daily/Weekly/`

Use these as copy-paste templates for structure & tone.

## Integration Points

**Personal Agent** (`/personal-agent`)
- Uses weekly retros as evidence for multi-week planning
- Maintains persistent project context

**Raw Inbox Memory** (`/raw-inbox-memory`)
- Durable personal facts → saved separately
- Weekly review is transient; only durable insights saved

**Knowledge Base** (Pasta)
- Weekly summaries indexed as KB evidence
- Used for "what did I accomplish in Sept?" queries

## Automation (Future)

Hook that runs Friday evening or Monday morning:
```bash
claude /weekly-review --date "2026-09-28" --author "Oscar"
```

Would:
1. Extract git commits for week
2. Read daily notes
3. Generate both retro files
4. Reorganize folder
5. Return summary link

## Checklist

- [ ] Extract git commits for week (`git log`)
- [ ] Review daily notes in `/Daily/2026-09/`
- [ ] Build detailed retro: Done/Next/Blockers per project
- [ ] Build weekly summary: Accomplishments, priorities, metrics
- [ ] Organize Daily folder: months → .archive, weeks → Weekly/
- [ ] Verify both files created + linked
- [ ] Optional: Share detailed retro with team (if applicable)

---

**Skill reference**: `/llm-wiki/skills/weekly-review/SKILL.md`
**KB guide**: [[Weekly-Review-Process]] (in Obsidian)
