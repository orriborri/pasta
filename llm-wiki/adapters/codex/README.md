# Codex adapter

Codex supports personal skills at:

```text
~/.codex/skills/<skill-name>/SKILL.md
```

Install both skills:

```bash
bash llm-wiki/skills/install-personal-skills.sh --codex
```

The same open `SKILL.md` bundles are used for Claude and Codex. Use `personal-agent` for personal-assistant workflows; durable personal information is delegated to `raw-inbox-memory`, which writes to Pasta's `0. Inbox/Raw/` rather than a Codex-specific memory mechanism.
