# Claude Code adapter

Claude Code discovers personal skills from:

```text
~/.claude/skills/<skill-name>/SKILL.md
```

Install both skills:

```bash
bash llm-wiki/skills/install-personal-skills.sh --claude
```

Use `/personal-agent` for personal-assistant sessions when Claude does not select it automatically. The `raw-inbox-memory` skill can be invoked directly when you explicitly want to persist a user statement.

Durable personal information is written to Pasta's `0. Inbox/Raw/`, not to a Claude-specific memory store.
