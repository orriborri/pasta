# Incremental upkeep workflow

This workflow is deliberately runtime-neutral. A scheduler may be cron, systemd, Hermes, CI, or a human-triggered shell.

1. Read the durable cursor with `vault.py cursor`.
2. Ask Pasta for a page of changes using MCP `get_changes` or CLI `kb changes --json`.
3. Run `vault.py plan` to turn changed records into entity-scoped jobs and candidate pages.
4. For every job, invoke any capable LLM with `prompts/maintain.md`.
5. The model may query Pasta for richer evidence/context, but returns only a patch JSON object.
6. Run `vault.py validate`; reject a patch that escapes its evidence boundary or target root.
7. Run `vault.py apply` for valid patches; it records durable completion receipts. Use `vault.py skip --plan PLAN --job-id JOB_ID --reason REASON` for explicit no-change decisions.
8. Advance each completed page with `vault.py advance --wiki WIKI --changes CHANGES --plan PLAN`. It verifies all jobs and output hashes before saving the cursor.
9. A bounded run (`run-upkeep.sh`) may stop with `has_more` true and advance page by page with `--allow-partial`; this is safe because Pasta orders changes by `(ingested_at, id)`, so late records with old or future source timestamps are not skipped.
10. Periodically run `vault.py audit`, resolve reported evidence IDs through Pasta, and feed the report to `prompts/garden.md` for global maintenance.
11. If PARA enhancement is explicitly requested, do it as a separate `llm-wiki` skill task: verify generated findings against Pasta, then edit the existing human note with citations. Do not send PARA edits through `vault.py` patch validation or apply.

The toolkit serializes apply/skip/advance with a POSIX advisory lock and rejects stale plans. Runners should still serialize whole maintenance cycles. If multiple jobs target the same page, finish one update and replan from the unchanged cursor so the next patch uses the new contents; explicitly skip already-covered jobs with a reason. Never bypass an edit conflict by editing the plan hashes.
