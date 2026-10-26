# LLM Wiki Toolkit

A runtime-agnostic maintenance layer for an LLM-managed Markdown wiki backed by Pasta evidence.

The toolkit deliberately does **not** contain an LLM client, scheduler, or agent framework. Hermes, Claude Code, Codex, Kiro, Cursor, a CI runner, or another orchestrator can all drive the same protocol.

## Boundary

- **Pasta** owns source ingestion, evidence records, deterministic relations, search, and provenance.
- **This toolkit** owns deterministic change planning, cursor state, patch validation, application, and audits.
- **The LLM runtime** owns semantic synthesis: deciding what the evidence means and how a page should change.

Generated wiki prose is context, not evidence. New factual claims must cite Pasta record IDs using `pasta:evidence:<record_id>`.

## Personal agent skills

The agent skills that drive this toolkit (`llm-wiki`, `personal-agent`, `raw-inbox-memory`, `daily-plan`, `weekly-review`) live in `agent/skills/`. See `agent/skills/README.md`; install with:

```bash
agent/install-skills.sh
```

Run `$daily-plan` or ask what to focus on today for a ready-to-review suggestion with up to three priorities and a concrete first action. It does not edit the daily note unless asked.

Raw capture flow:

```text
Claude / Codex / other agent
          |
          | exact user statement
          v
0. Inbox/Raw/*.md
          |
          | Pasta vault sync
          v
Pasta evidence
          |
          v
LLM Wiki maintenance
          |
          v
vault (wiki root)
```

Pasta deliberately indexes only the `0. Inbox/Raw/` subtree of the operational inbox. Generated wiki proposals are grouped by JDex ID and carry `llm_wiki: 1`. `VaultFetcher` excludes these pages from evidence ingestion regardless of their folder.

The capture helper rejects obvious credential-like content by default and never silently claims persistence when a write fails.

## Incremental cycle

```text
Pasta get_changes / kb changes
          |
          v
    vault.py plan
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
 vault.py validate/apply
          |
          v
    vault.py advance
```

The wiki uses the existing private JDex registry. `init --registry PATH` imports
it to `.llm-wiki/jdex-registry.json` without creating PARA folders. IDs must be
exact `NN.NN` strings with unique IDs and nonempty labels. For publication an
entry must explicitly set `state` to `approved` or `management` and
`publication_enabled` to true. Audit exports do not automatically grant approval.

Generated proposals live under `.llm-wiki/generated/NN.NN/title.md`. Each patch
and its Markdown frontmatter must declare the same `jd_id`. The registry hash
binds the plan through validation, application and cursor advancement; edits to
the registry require replanning. Uncertain classifications must be skipped with
a review reason. No nearest-match fallback or number allocation is permitted.

Example private registry (fictional entry):
```json
{"entries":[{"jd_id":"41.13","label":"Training","state":"approved","publication_enabled":true}]}
```

Human JDex originals remain untouched. Previously generated PARA pages are not
automatically moved; review and regenerate them with approved JD IDs. The
hidden generated area is a review workspace grouped by JD ID, not a second
canonical document tree. The physical NAS vault remains organized by area,
category, and ID. Publishing into human notes is separate reviewed work.

Deploy the updated fetcher before moving generated pages into indexed folders. Existing evidence records are retained; this exclusion prevents future ingestion and does not purge previously indexed prose.

## Quick start

```bash
python llm-wiki/vault.py init --wiki ~/vault --registry /path/to/private/jdex-registry.json

CURSOR="$(python llm-wiki/vault.py cursor --wiki ~/vault)"
kb changes --json ${CURSOR:+--cursor "$CURSOR"} > /tmp/wiki-changes.json

python llm-wiki/vault.py plan \
  --wiki ~/vault \
  --changes /tmp/wiki-changes.json \
  --out /tmp/wiki-plan.json
```

Give one job from `/tmp/wiki-plan.json` plus `prompts/maintain.md` to your chosen LLM. The model may call Pasta MCP tools (`get_context`, `get_evidence`, `get_timeline`, `search_knowledge`) and must return JSON matching `schemas/patch.schema.json`.

```bash
python llm-wiki/vault.py validate \
  --wiki ~/vault \
  --plan /tmp/wiki-plan.json \
  --patch /tmp/wiki-patch.json

python llm-wiki/vault.py apply \
  --wiki ~/vault \
  --plan /tmp/wiki-plan.json \
  --patch /tmp/wiki-patch.json
```

After all jobs from the change page have succeeded, advance the durable cursor:

```bash
python llm-wiki/vault.py advance \
  --wiki ~/vault \
  --changes /tmp/wiki-changes.json \
  --plan /tmp/wiki-plan.json
```

Each applied patch records a durable completion receipt. Jobs needing no page change must be explicitly completed with a reason:

```bash
python llm-wiki/vault.py skip --wiki ~/vault \
  --plan /tmp/wiki-plan.json --job-id JOB_ID --reason "No durable change"
```

`advance` requires the matching plan and refuses unfinished jobs or changed output pages. Advance after each completed page, including a page with `has_more: true`, then fetch the next page with the saved cursor. Plans snapshot page hashes; an intervening edit requires replanning. Exact patch retries recover a page write interrupted before its completion receipt. Writes use a POSIX advisory lock, reject symlinks below the wiki root, and preserve unrelated pages. Manual editors do not participate in that lock; hash checks detect edits observed before replacement.

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

- `init` — create the wiki root, import JDex routing, and initialize cursor state.
- `cursor` — print the opaque Pasta change cursor.
- `plan` — turn a Pasta change page into entity-scoped LLM jobs.
- `validate` — enforce path safety, entity ownership, and evidence boundaries.
- `apply` — atomically write a validated Markdown page.
- `advance` — verify completion receipts and store the next Pasta cursor.
- `skip` — record an explicit no-change decision with its reason.
- `audit` — report uncited pages, missing entity metadata, duplicate entity ownership, and all referenced evidence IDs.

The `audit` report covers generated pages under `.llm-wiki/generated/`. Resolve its evidence ID list through Pasta `get_evidence` to detect stale or broken references.

## Design rules

1. Pasta evidence is authoritative; wiki prose is derived.
2. Semantic similarity may help retrieval but never becomes evidence by itself.
3. Scripts decide **when/where** work is needed; models decide **what/how** to synthesize.
4. Contradictions are preserved and explained, not silently overwritten.
5. The orchestration contract is JSON + Markdown, not a specific LLM SDK.
6. Cursor advancement happens only after all jobs in a change page succeed.
