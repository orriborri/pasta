# Guides & Processes

Operational and workflow documentation for Pasta and the personal knowledge system.

## Personal Workflows

### Weekly Review Process
**File:** `weekly-review-workflow.md`

Consolidated workflow for generating weekly retros from git history + daily notes. Creates two files:
1. Detailed retro (`2. Areas/Infra Updates/weekly-oscar-*.md`) — project breakdown, next steps
2. Weekly summary (`Daily/Weekly/Week-*.md`) — accomplishments, priorities, metrics

Plus organizes Daily folder by month + week.

**Skill:** `/llm-wiki/skills/weekly-review/SKILL.md`

**KB Reference:** `3. Resources/Weekly-Review-Process.md` (in Obsidian Readpeak vault)

**Quick start:**
```bash
/weekly-review
```

**Manual workflow:**
1. Extract git commits for week
2. Read daily notes
3. Generate detailed retro + summary
4. Reorganize `Daily/` folder

**Time:** ~30 minutes

---

## Related Processes

- **Personal Agent** — Cross-session knowledge system & evidence retrieval
- **Raw Inbox Memory** — Durable personal fact capture (separate from transient retros)
- **Daily Notes** — Input for weekly reviews (structured by month + week)

---

## File Locations

### In this repo
- Skills: `/llm-wiki/skills/*/SKILL.md`
- Specs: `/`.kiro/specs/`
- Guides: `/`.kiro/guides/`

### In Obsidian Readpeak vault
- Detailed retros: `2. Areas/Infra Updates/weekly-oscar-*.md`
- Weekly summaries: `0. Inbox/Daily/Weekly/Week-*.md`
- Process guides: `3. Resources/*.md`
- Daily notes: `0. Inbox/Daily/2026-09/` (organized by month)

---

**Last updated:** 2026-09-28
