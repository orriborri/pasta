## 1. Infrastructure

- [x] 1.1 Add `lancedb` crate dependency to backend, create `.lancedb/` directory and table schema (id, source, path, participants, date, content, vector)
- [x] 1.2 Create `.history/` directory structure and `_sync_state.json` module
- [x] 1.3 Add embedding module (OpenAI text-embedding-3-small, with Ollama fallback)
- [x] 1.4 Add chunking module (split content into ~500 token segments with overlap)
- [x] 1.5 Add `history-sync` as a native schedule in the backend (daily)
- [x] 1.6 Create `repo-index.toml` config for selecting which repo files to index

## 2. Slack history sync

- [x] 2.1 Fetch all DM conversations, paginate history (6 months), write to `.history/slack/dm/<person>.md`
- [x] 2.2 Fetch joined channels, paginate history (90 days), write to `.history/slack/channels/<channel>.md`
- [x] 2.3 Rate-limit aware with progress tracking and resumption
- [x] 2.4 Incremental mode: only fetch messages newer than last sync

## 3. Gmail history sync

- [x] 3.1 Fetch relevant threads (6 months, exclude GitLab/promotions/social), write to `.history/gmail/<YYYY-MM>/<slug>.md`
- [x] 3.2 Incremental mode: only fetch threads newer than last sync

## 4. Linear history sync

- [x] 4.1 Fetch all issues (created/assigned/commented) with comments, write to `.history/linear/<identifier>.md`
- [x] 4.2 Incremental mode: only re-fetch issues updated since last sync

## 5. Wiki & repos sync

- [x] 5.1 Copy all wiki pages from `/home/orre/ReadPeak/wiki/` to `.history/wiki/`
- [x] 5.2 Sync configured repo files per `repo-index.toml` to `.history/repos/<repo>/`
- [x] 5.3 Detect changed files (mtime or hash) to avoid re-embedding unchanged content

## 6. LanceDB indexing

- [x] 6.1 After each source sync, chunk new/changed markdown files and generate embeddings
- [x] 6.2 Upsert chunks into LanceDB table with metadata (source, path, participants, date)
- [x] 6.3 Delete stale chunks when source files are removed

## 7. Search interface

- [x] 7.1 Create `pasta-search` CLI binary that queries LanceDB and returns ranked results
- [x] 7.2 Add `Search` IPC command so agents connected via backend can query the index
- [x] 7.3 Support filters: --source, --participant, --after, --limit

## 8. Verify

- [x] 8.1 Build workspace, test wiki indexing end-to-end (sync → chunk → embed → search)
