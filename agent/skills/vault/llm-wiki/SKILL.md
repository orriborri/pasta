---
name: llm-wiki
description: Maintain this vault's Pasta-backed LLM wiki and curate PARA notes
  from it. Use when the user asks to update or audit the wiki under .llm-wiki/,
  run the vault.py cursor/plan/validate/apply/advance cycle, or improve a PARA
  note from generated wiki pages. Not for plain note search, note creation, or
  saving new personal facts.
license: UNLICENSED
metadata:
  id: ee9e0d76-5135-4cd1-9ca8-d1532df0bd53
  author: Oscar Henriksson <oscar.henriksson91@gmail.com>
  terum-category: workflow
---

# LLM Wiki Maintenance

The compiler is `vault.py` in the Pasta repo checkout (`~/code/pasta/llm-wiki/vault.py`). The vault root comes from `~/.pasta/config.toml` → `[general].vault_path`; generated pages live under `.llm-wiki/generated/`.

## Pick the right tool

- Searching or reading notes: Pasta `search_knowledge`, or read the files directly. Never `vault.py`.
- Creating or editing an ordinary note: write the file (or use the Obsidian CLI). Never `vault.py`.
- Saving a new personal statement: the `raw-inbox-memory` skill writes it to `0. Inbox/Raw/`. Never write it into the wiki.
- Wiki upkeep: `vault.py`, following the cycle below.

## Wiki upkeep cycle

Follow `~/code/pasta/llm-wiki/workflows/incremental.md`. The rules that matter:

1. `vault.py cursor`, then fetch changes with Pasta `get_changes`.
2. `vault.py plan` to get jobs; produce one patch per job with `prompts/maintain.md`.
3. `vault.py validate` every patch. If validation fails, stop: do not `apply`, do not `advance`, and report the validator's message verbatim.
4. `vault.py apply` only validated patches.
5. `vault.py advance --changes <page.json>` only after every job on that page is applied. When stopping with `has_more` still true (bounded runs), advance page by page with `--allow-partial`; never advance past a page with a rejected or unapplied patch.

The nightly `pasta-llm-wiki.timer` runs this cycle unattended via `~/code/pasta/llm-wiki/run-upkeep.sh`, with logs under `~/.local/state/pasta/llm-wiki/runs/`.

Every factual claim in wiki prose cites its evidence as `pasta:evidence:<record_id>`. Generated prose is context, not evidence.

## PARA curation

Only when the user explicitly asks to improve a PARA note from the wiki:

1. Read the relevant pages under `.llm-wiki/generated/`.
2. Verify each claim against Pasta evidence; drop what you cannot verify.
3. Edit the existing canonical note with a concise synthesis and nearby `pasta:evidence:` citations. Do not send PARA edits through `vault.py`.

Placement: `1. Projects/` for active outcome-based commitments, `2. Areas/` for ongoing responsibilities, `3. Resources/` for reusable reference, `4. Archive/` for inactive material. Do not copy issue-by-issue generated pages into Projects, and do not replace a human-written note with generated prose.
