# Agent skills

Source of truth for the Pasta agent skills. `agent/install-skills.sh` copies them to where agents load them.

## Global (`global/` → `~/.claude/skills`, `~/.codex/skills`)

Useful in any session that can reach Pasta.

- `personal-agent` — retrieve prior personal context from Pasta instead of guessing.
- `raw-inbox-memory` — write durable user statements to the vault's `0. Inbox/Raw/` via `scripts/capture.py`.

## Vault (`vault/` → `<vault>/.claude/skills`, linked from `<vault>/.agents/skills`)

Only meaningful inside the Readpeak vault. The vault path comes from `~/.pasta/config.toml` → `[general].vault_path`.

- `daily-plan` — today's top three priorities from the daily note, task board, projects, and Pasta.
- `weekly-review` — weekly retro from git history and daily notes.
- `llm-wiki` — the `vault.py` wiki upkeep cycle and PARA curation from generated pages.

## Install

```bash
agent/install-skills.sh            # global + vault
agent/install-skills.sh --global   # global only
agent/install-skills.sh --vault    # vault only
agent/install-skills.sh --force    # overwrite copies that differ from source
```

Unchanged copies are skipped. Retired global skills (`pasta-vault`, `daily-plan`) are removed from `~/.claude/skills` and `~/.codex/skills`.

## Flow

```text
user statement → raw-inbox-memory → 0. Inbox/Raw/ → Pasta evidence
Pasta evidence → llm-wiki (vault.py) → .llm-wiki/generated/ → curated PARA notes
personal-agent / daily-plan / weekly-review read Pasta and the vault
```
