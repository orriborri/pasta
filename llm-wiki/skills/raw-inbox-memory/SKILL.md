---
name: raw-inbox-memory
description: Externalize durable personal information into Pasta's append-only raw inbox instead of agent-specific memory. Use whenever the user asks to remember, save, note, keep, or recall something later, or states a stable preference, decision, relationship, or personal fact that should survive future sessions.
---

# Raw Inbox Memory

Treat Pasta's raw inbox as the only durable personal-memory write target.

## Core rule

Do not use product-specific or model-specific long-term memory as the durable store for personal information. Do not claim that you will remember something merely because it appeared in the conversation.

When information should survive this session, write the user's **raw statement** to `0. Inbox/Raw/` using the bundled capture script.

The current conversation can still be used normally as working context. This rule concerns cross-session persistence.

## What to capture

Capture when the user explicitly asks to remember/save/note something.

Also capture stable user-provided information when it is clearly useful in future personal-assistant work, for example:
- preferences;
- decisions;
- commitments;
- relationships/names the user explicitly provides;
- recurring routines;
- durable project or household context;
- facts the user expects an assistant to know later.

Do not capture:
- passwords, authentication codes, API keys, private keys, recovery codes, or other credentials;
- inferred sensitive traits or conclusions the user did not state;
- speculative interpretations;
- temporary conversational details that have no likely future value;
- whole conversation transcripts when one user statement is sufficient.

If information is sensitive but the user explicitly asks to preserve it, prefer asking whether they want it written to the raw inbox rather than silently storing it.

## Capture procedure

Preserve the user's wording. Do not rewrite a raw capture into a confident fact.

Resolve `scripts/capture.py` relative to the directory containing this `SKILL.md`, then run:

```bash
python3 <skill-dir>/scripts/capture.py \
  --kind <fact|preference|decision|commitment|relationship|routine|note> \
  --source <runtime-name> \
  --text '<the user-provided statement verbatim>'
```

The script resolves the Pasta vault from:
1. `--vault`;
2. `PASTA_VAULT_PATH`;
3. `~/.pasta/config.toml` → `[general].vault_path`.

Use `--context` only for neutral provenance such as the current project or task. Do not put inferred personal conclusions in context.

If capture fails, tell the user it was not persisted. Never pretend storage succeeded.

## After capture

Do not immediately turn the raw capture into curated wiki knowledge yourself. Pasta ingests `0. Inbox/Raw/` as raw evidence; the separate wiki maintenance workflow performs synthesis later.

Only run an immediate Pasta vault sync when the user needs the new capture to become searchable right away.
