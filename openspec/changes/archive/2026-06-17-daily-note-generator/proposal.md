## Why

The Obsidian vault has grown organically — documents may be in wrong PARA folders, missing backlinks to related content, or have stale/broken links. With a vector store of all synced content, we can semantically verify the entire vault: ensure links are correct, add missing connections, and verify PARA placement.

## What Changes

- Phase 1: **Link verification** — scan every vault document, use vector similarity to find related documents, add/fix `[[backlinks]]` in a `## Related` section
- Phase 2: **PARA audit** — for each document, verify it's in the correct PARA folder based on its content. Suggest moves for misplaced documents.
- Phase 3: **Daily activity feed** — append today's activity to the daily note with backlinks
- Add `--organize` command that runs phases 1+2
- Add `--daily` command that runs phase 3

## Capabilities

### New Capabilities
- `vault-link-repair`: Scans all vault documents, uses vector similarity to find and add missing `[[backlinks]]` to related content
- `vault-para-audit`: Verifies each document is in the correct PARA category based on content similarity and suggests moves
- `daily-note-activity`: Generates daily activity section with backlinks

### Modified Capabilities

## Impact

- New module: `crates/backend/src/vault_organize.rs`
- Uses `simple_index::search` for semantic matching
- Reads/writes documents in `Obsidian/Readpeak/` vault
- Must index vault PARA documents into the vector store first
