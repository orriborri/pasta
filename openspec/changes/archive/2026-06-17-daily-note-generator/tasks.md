## 1. Index Vault into Vector Store

- [x] 1.1 Scan all `.md` files in `1. Projects/`, `2. Areas/`, `3. Resources/` into the vector store
- [x] 1.2 Include document title, path, and PARA category as metadata
- [x] 1.3 Skip `4. Archive/` and `.history/`

## 2. Link Verification & Repair (Phase 1)

- [x] 2.1 For each vault document, extract its content and search vectors for top-5 similar documents
- [x] 2.2 Check if those related documents are already linked via `[[backlinks]]`
- [x] 2.3 If missing links found (similarity > 0.75), add a `## Related` section with new backlinks
- [x] 2.4 Don't duplicate existing links — only add missing ones
- [x] 2.5 Track processed files to avoid re-scanning unchanged docs

## 3. PARA Audit (Phase 2)

- [x] 3.1 For each document, find its top-3 most similar documents
- [x] 3.2 If majority of similar docs are in a different PARA category, flag as potentially misplaced
- [x] 3.3 Generate a report: `vault-audit.md` listing suggested moves with reasoning
- [x] 3.4 Don't auto-move — only suggest (user reviews the report)

## 4. Daily Activity Feed (Phase 3)

- [x] 4.1 Collect today's synced items from `.history/` (by last_updated date)
- [x] 4.2 For each item, find related PARA docs via vector search
- [x] 4.3 Format as activity section with `[[backlinks]]`
- [x] 4.4 Append/replace `## 🔗 Activity Feed` in today's daily note
- [x] 4.5 Create daily note if missing

## 5. CLI Integration

- [x] 5.1 Add `--organize` flag (runs phase 1 + 2)
- [x] 5.2 Add `--daily` flag (runs phase 3)
- [x] 5.3 Both respect the `--local` flag (no API calls needed)

## 6. Vault Organization

- [x] 6.1 Scan `0. Inbox/Notes/` for unprocessed notes (no `related:` in frontmatter)
- [x] 6.2 For each inbox note, search vector store for top-3 similar PARA documents
- [x] 6.3 Append `## Related` section with `[[backlinks]]` to similar documents
- [x] 6.4 Add `related: true` to frontmatter to mark as processed
- [x] 6.5 Suggest PARA placement in the Related section (e.g., "Consider moving to → [[1. Projects/Fix pipelines]]")
