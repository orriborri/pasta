---
name: personal-agent
description: Retrieve the user's prior personal context from Pasta instead of
  guessing or relying on model memory. Use when the user asks what they said,
  decided, or planned before, refers to people, projects, or household matters
  from earlier sessions, or asks for follow-ups that depend on past context.
license: UNLICENSED
metadata:
  id: a1b2c3d4-e5f6-4a5b-9c8d-7e6f5a4b3c2d
  author: Oscar Henriksson <oscar.henriksson91@gmail.com>
  terum-category: workflow
---

# Personal Context from Pasta

Use Pasta as the external personal knowledge system. Treat model/session memory as working context only, not as the authoritative long-term store.

## Read path

When prior context could materially improve the task:
1. search Pasta rather than guessing;
2. use structured evidence/context tools when available;
3. prefer current evidence and explicit user statements over synthesized wiki prose;
4. distinguish facts, past states, and current states.

Useful Pasta MCP tools include:
- `search_knowledge`;
- `get_entity`;
- `get_related`;
- `get_context`;
- `get_timeline`;
- `get_evidence`;
- `get_changes`.

### Background from the compiled wiki

Pasta search does not index the compiled wiki, so read it directly when you need orientation on a person, issue, or system. It lives at `<vault>/.llm-wiki/generated/` (vault path: `~/.pasta/config.toml` → `[general].vault_path`):
- `Linear Issues/<key>.md`, lowercase issue key, for example `ops-233.md`;
- `Profiles/<name-slug>.md` for people and organizations;
- `Systems/` for services, bots, and infrastructure; `Topics/` for channel and theme digests.

Wiki prose is a synthesis and may be stale. Before stating a wiki claim as fact, resolve its `pasta:evidence:<record_id>` citation with `get_evidence`; drop or flag claims you cannot verify. Wiki and PARA maintenance belong to the vault's `llm-wiki` skill.

## Write path

When the user provides information that should persist across sessions, follow the sibling `raw-inbox-memory` skill.

The durable flow is:

```text
user statement
     ↓
0. Inbox/Raw/       (verbatim/raw capture)
     ↓
Pasta evidence
     ↓
LLM Wiki compiler  (generated working pages)
     ↓  explicit curation after evidence verification
existing PARA notes  (human-readable summaries)
```

Do not directly promote a new personal statement into a curated PARA page during the same conversational turn unless the user explicitly asks for that edit.

## Behavior

- Be useful with the context available now; persistence is a separate concern.
- Never say you remember a cross-session fact unless you retrieved it from Pasta or another explicit store.
- Never infer sensitive personal attributes and save them as facts.
- Prefer small raw captures over conversation dumps.
- If the user corrects an older fact, capture the correction as new raw evidence rather than deleting history.
- If retrieval and the user's current statement conflict, ask or represent the change over time instead of silently choosing one.
