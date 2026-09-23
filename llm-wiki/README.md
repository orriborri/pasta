# LLM Wiki Toolkit

A runtime-agnostic maintenance layer for an LLM-managed Markdown wiki backed by Pasta evidence.

The toolkit deliberately does **not** contain an LLM client, scheduler, or agent framework. Hermes, Claude Code, Codex, Kiro, Cursor, a CI runner, or another orchestrator can all drive the same protocol.

## Boundary

- **Pasta** owns source ingestion, evidence records, deterministic relations, search, and provenance.
- **This toolkit** owns deterministic change planning, cursor state, patch validation, application, and audits.
- **The LLM runtime** owns semantic synthesis: deciding what the evidence means and how a page should change.

Generated wiki prose is context, not evidence. New factual claims must cite Pasta record IDs using `pasta:evidence:<record_id>`.

## Incremental cycle

```text
Pasta get_changes / kb changes
          |
          v
    wiki.py plan
          |
          v
    portable JSON jobs
          |
          v
       ANY LLM
          |
          v
    portable JSON patch
          |
          v
 wiki.py validate/apply
          |
          v
    wiki.py advance
```

For an Obsidian vault used by Pasta, `Knowledge/` is a good wiki root because the current Pasta `VaultFetcher` only ingests the configured PARA/task/person roots. Keep the compiled wiki outside those evidence roots so generated prose does not feed back into the evidence store.

## Quick start

```bash
python llm-wiki/wiki.py init --wiki ~/vault/Knowledge

CURSOR="$(python llm-wiki/wiki.py cursor --wiki ~/vault/Knowledge)"
kb changes --json ${CURSOR:+--cursor "$CURSOR"} > /tmp/wiki-changes.json

python llm-wiki/wiki.py plan \
  --wiki ~/vault/Knowledge \
  --changes /tmp/wiki-changes.json \
  --out /tmp/wiki-plan.json
```

Give one job from `/tmp/wiki-plan.json` plus `prompts/maintain.md` to your chosen LLM. The model may call Pasta MCP tools (`get_context`, `get_evidence`, `get_timeline`, `search_knowledge`) and must return JSON matching `schemas/patch.schema.json`.

```bash
python llm-wiki/wiki.py validate \
  --wiki ~/vault/Knowledge \
  --plan /tmp/wiki-plan.json \
  --patch /tmp/wiki-patch.json

python llm-wiki/wiki.py apply \
  --wiki ~/vault/Knowledge \
  --plan /tmp/wiki-plan.json \
  --patch /tmp/wiki-patch.json
```

After all jobs from the change page have succeeded, advance the durable cursor:

```bash
python llm-wiki/wiki.py advance \
  --wiki ~/vault/Knowledge \
  --changes /tmp/wiki-changes.json
```

If Pasta reports `has_more: true`, fetch/process the next change page before advancing the durable cursor. The toolkit refuses this by default.

## Wiki page contract

Pages use normal Markdown with small YAML frontmatter:

```markdown
---
llm_wiki: 1
entities:
  - project:recommendations
evidence:
  - slack-C123-1712345678
  - linear-RP-123
---

# Recommendations

The service is migrating to EKS. [source](pasta:evidence:linear-RP-123)
```

`entities` let the deterministic planner find candidate pages later. `evidence` is an index; actual factual text should also carry inline `pasta:evidence:` citations.

## Commands

- `init` — create the wiki root, standard section directories, and cursor state.
- `cursor` — print the opaque Pasta change cursor.
- `plan` — turn a Pasta change page into entity-scoped LLM jobs.
- `validate` — enforce path safety, entity ownership, and evidence boundaries.
- `apply` — atomically write a validated Markdown page.
- `advance` — store the next Pasta cursor after a successful cycle.
- `audit` — report uncited pages, missing entity metadata, duplicate entity ownership, and all referenced evidence IDs.

The `audit` evidence ID list can be resolved in bulk through Pasta `get_evidence` to detect stale/broken references without granting the LLM authority to invent replacements.

## Design rules

1. Pasta evidence is authoritative; wiki prose is derived.
2. Semantic similarity may help retrieval but never becomes evidence by itself.
3. Scripts decide **when/where** work is needed; models decide **what/how** to synthesize.
4. Contradictions are preserved and explained, not silently overwritten.
5. The orchestration contract is JSON + Markdown, not a specific LLM SDK.
6. Cursor advancement happens only after all jobs in a change page succeed.
