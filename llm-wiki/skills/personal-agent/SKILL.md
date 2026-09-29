---
name: personal-agent
description: Act as a grounded personal assistant using Pasta for cross-session context and the raw inbox for new durable information. Use for personal planning, follow-ups, projects, household context, decisions, preferences, or any task where prior personal context may matter.
---

# Personal Agent

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

The compiled wiki can be used as a navigation/synthesis layer, but Pasta evidence is authoritative.

## PARA maintenance

When the user asks to improve the PARA vault from the LLM wiki, inspect relevant pages under `.llm-wiki/generated/`, verify proposed claims against Pasta evidence, and update the existing canonical PARA note with a concise synthesis and nearby evidence citations. Keep `1. Projects/` for active outcome-based commitments, `2. Areas/` for ongoing responsibilities, `3. Resources/` for reusable reference, and `4. Archive/` for inactive material. Do not copy issue-by-issue generated pages into Projects or replace a human note with generated prose.

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
