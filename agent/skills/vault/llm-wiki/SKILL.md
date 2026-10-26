---
name: llm-wiki
description: Maintain this vault's Pasta-backed LLM wiki and curate JDex notes
  from it. Use when the user asks to update or audit the wiki under .llm-wiki/,
  run the vault.py cursor/plan/validate/apply/advance cycle, or improve a JDex
  note from generated wiki pages. Not for plain note search, note creation, or
  saving new personal facts.
license: UNLICENSED
metadata:
  id: ee9e0d76-5135-4cd1-9ca8-d1532df0bd53
  author: Oscar Henriksson <oscar.henriksson91@gmail.com>
  terum-category: workflow
---

# LLM Wiki Maintenance

The compiler is `vault.py` in the Pasta repo checkout (`~/code/pasta/llm-wiki/vault.py`). The vault root comes from `~/.pasta/config.toml` → `[general].vault_path`; generated proposals live under `.llm-wiki/generated/NN.NN/` using the private JDex registry.

## Pick the right tool

- Searching or reading notes: Pasta `search_knowledge`, or read the files directly. Never `vault.py`.
- Creating or editing an ordinary note: write the file (or use the Obsidian CLI). Never `vault.py`.
- Saving a new personal statement: the `raw-inbox-memory` skill writes it to `0. Inbox/Raw/`. Never write it into the wiki.
- Wiki upkeep: `vault.py`, following the cycle below.

## Wiki upkeep cycle

Follow `~/code/pasta/llm-wiki/workflows/incremental.md`. The rules that matter:

1. `vault.py cursor`, then fetch changes with Pasta `get_changes`.
2. `vault.py plan` to get jobs; use their enabled JDex routing entries and `prompts/maintain.md`. Each patch and frontmatter declares one matching `jd_id`. Skip uncertain routing with a review reason.
3. `vault.py validate` every patch. If validation fails, stop: do not `apply`, do not `advance`, and report the validator's message verbatim.
4. `vault.py apply` only validated patches.
5. `vault.py advance --changes <page.json>` only after every job on that page is applied. When stopping with `has_more` still true, advance completed pages only. Each job needs a verified apply or explicit skip receipt.

The nightly `pasta-llm-wiki.timer` runs this cycle unattended via `~/code/pasta/llm-wiki/run-upkeep.sh`, with logs under `~/.local/state/pasta/llm-wiki/runs/`.

Every factual claim in wiki prose cites its evidence as `pasta:evidence:<record_id>`. Generated prose is context, not evidence.

## JDex curation

For an explicitly requested canonical-note update, retrieve its approved JDex
entry and the relevant generated proposal. Verify claims through Pasta evidence,
then propose a cited update to that ID's existing canonical note. Preserve human
content and source files. Uncertain destinations remain review items. Placement
uses the existing area/category/ID hierarchy, not model-created categories.

Initialize the toolkit with `init --wiki ROOT --registry PRIVATE_REGISTRY`.
Publication requires an approved or management entry with publication_enabled
set to true. Registry edits invalidate outstanding plans; replan before writing.
