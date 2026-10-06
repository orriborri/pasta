# Incremental upkeep workflow

This workflow is deliberately runtime-neutral. A scheduler may be cron, systemd, Hermes, CI, or a human-triggered shell.

1. Read the durable cursor with `wiki.py cursor`.
2. Ask Pasta for a page of changes using MCP `get_changes` or CLI `kb changes --json`.
3. Run `wiki.py plan` to turn changed records into entity-scoped jobs and candidate pages.
4. For every job, invoke any capable LLM with `prompts/maintain.md`.
5. The model may query Pasta for richer evidence/context, but returns only a patch JSON object.
6. Run `wiki.py validate`; reject a patch that escapes its evidence boundary or target root.
7. Run `wiki.py apply` for valid patches; it records durable completion receipts. Use `wiki.py skip --plan PLAN --job-id JOB_ID --reason REASON` for explicit no-change decisions.
8. Advance this completed page with `wiki.py advance --wiki WIKI --changes CHANGES --plan PLAN`. It verifies all jobs and output hashes before saving the cursor.
9. If `has_more` is true, fetch the next page with the saved cursor and repeat. Completed pages are checkpoints, so failures do not require replaying the entire backlog.
10. Periodically run `wiki.py audit`, resolve reported evidence IDs through Pasta, and feed the report to `prompts/garden.md` for global maintenance.

The toolkit serializes apply/skip/advance with a POSIX advisory lock and rejects stale plans. Runners should still serialize whole maintenance cycles. If multiple jobs target the same page, finish one update and replan from the unchanged cursor so the next patch uses the new contents; explicitly skip already-covered jobs with a reason. Never bypass an edit conflict by editing the plan hashes.
