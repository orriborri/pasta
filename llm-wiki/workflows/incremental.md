# Incremental upkeep workflow

This workflow is deliberately runtime-neutral. A scheduler may be cron, systemd, Hermes, CI, or a human-triggered shell.

1. Read the durable cursor with `wiki.py cursor`.
2. Ask Pasta for a page of changes using MCP `get_changes` or CLI `kb changes --json`.
3. Run `wiki.py plan` to turn changed records into entity-scoped jobs and candidate pages.
4. For every job, invoke any capable LLM with `prompts/maintain.md`.
5. The model may query Pasta for richer evidence/context, but returns only a patch JSON object.
6. Run `wiki.py validate`; reject a patch that escapes its evidence boundary or target root.
7. Run `wiki.py apply` for valid patches.
8. If `has_more` is true, continue with Pasta using `next_cursor` as the transient paging cursor and repeat steps 2–7.
9. Only after the whole logical cycle succeeds, persist the final cursor with `wiki.py advance`.
10. Periodically run `wiki.py audit`, resolve reported evidence IDs through Pasta, and feed the report to `prompts/garden.md` for global maintenance.

A production runner should use a single-flight lock around steps 1–9 so two cycles cannot advance the cursor concurrently.
