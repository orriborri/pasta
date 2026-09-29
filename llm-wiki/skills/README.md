# Personal Agent Skills

Skills for the personal knowledge system. Use with Claude Code via `/skill-name` or programmatically.

## Available Skills

### personal-agent
Act as a grounded personal assistant using Pasta for cross-session context. Use for planning, follow-ups, projects, decisions, or tasks where prior context matters.

**File:** `personal-agent/SKILL.md`

**Read path:** Search Pasta, use structured evidence, distinguish facts from synthesis

**Write path:** Follow `raw-inbox-memory` skill for durable captures

### raw-inbox-memory
Capture durable personal facts, decisions, preferences, and commitments for persistence across sessions.

**File:** `raw-inbox-memory/SKILL.md`

**Use for:** Explicit user requests to remember, durable preferences, decisions, commitments

**Flow:**
```
User statement
    ↓
0. Inbox/Raw/ (verbatim capture)
    ↓
Pasta evidence
    ↓
LLM Wiki compiler
    ↓
Curated PARA notes (optional)
```

### weekly-review
Generate weekly retrospectives from git history + daily notes. Consolidates work into structured format and organizes daily folder.

**File:** `weekly-review/SKILL.md`

**Generates:**
1. Detailed retro (`2. Areas/Infra Updates/weekly-oscar-YYYY-MM-DD.md`)
2. Weekly summary (`0. Inbox/Daily/Weekly/Week-YYYY-MM-DD-to-YYYY-MM-DD.md`)
3. Organized Daily folder (by month + week)

**Workflow:**
- Extract git commits for week
- Read daily notes
- Build Done/Next/Blockers structure
- Organize folder

**Time:** ~30 minutes

### daily-plan
*[Placeholder for daily planning skill, if implemented]*

---

## Installation

Run the installer:
```bash
bash install-personal-skills.sh
```

This registers skills with Claude Code so they appear in `/` completions.

## Integration

All skills use the same:
- **Knowledge store:** Pasta (KB system)
- **Evidence base:** Raw inbox captures
- **PARA vault:** Curated personal notes in Obsidian

**Flow:**
```
personal-agent → reads Pasta
    ↑
    |
raw-inbox-memory → writes evidence
    ↑
    |
weekly-review → aggregates + organizes
    ↓
PARA vault (curated by human)
```

## Related Documentation

- Detailed guide: `/`.kiro/guides/weekly-review-workflow.md`
- KB reference: `3. Resources/Weekly-Review-Process.md` (in Obsidian)
- Personal agent: `personal-agent/SKILL.md`
- Raw inbox: `raw-inbox-memory/SKILL.md`

---

**Last updated:** 2026-09-28
