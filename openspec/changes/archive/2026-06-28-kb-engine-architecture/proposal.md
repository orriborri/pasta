## Why

Pasta has outgrown its original scope. The knowledge base (ingestion, pre-processing, embedding, search) is now the most complex subsystem but it's trapped inside a vault-maintenance cycle of a personal daemon. It needs:
- A proper pre-processing pipeline (normalize, dedupe, entity extraction, summarization)
- Scalable storage (Arrow/Parquet for raw data, not just markdown)
- Multiple search modes (vector + full-text + structured)
- Ability to run standalone (CLI, server, or scheduled)
- Path to team use and AWS deployment

Splitting into a dedicated `kb-engine` project lets both pasta and the KB evolve independently.

## What Changes

- **New project: `kb-engine`** — standalone knowledge base engine with ingestion pipeline, storage, and search
- **Extract from pasta**: move `history/`, `embedder`, `indexer`, MCP search tools into kb-engine
- **Pasta becomes thin orchestrator**: calls `kb-engine sync` and `kb-engine search`, keeps daemon/TUI/agents
- **New pre-processing pipeline**: normalize → dedupe → entity extraction → summarize → chunk → embed
- **Storage upgrade**: Arrow/Parquet for raw data, LanceDB for vectors, Tantivy for full-text
- **New sources**: Google Docs, Calendar deep sync

## Capabilities

### New Capabilities
- `ingestion-pipeline`: Multi-stage pre-processing pipeline with normalize/dedupe/extract/summarize/chunk/embed
- `arrow-storage`: Columnar storage for raw ingested data using Arrow/Parquet
- `full-text-search`: Tantivy-based keyword search alongside vector search
- `kb-api`: Standalone query API (MCP + CLI + HTTP)

### Modified Capabilities
- `state-persistence`: Sync state moves to kb-engine, pasta references it externally

## Impact

- Same `pasta/` workspace — new `kb-*` crates, no new repo, no code move
- Pasta keeps: backend (daemon), tui, common
- New crates: kb-core, kb-fetchers, kb-pipeline, kb-storage, kb-search, kb-cli
- New binaries: `kb`, `kb-mcp`
- Pasta's `history/` modules gradually replaced by kb-engine library calls
- New fetchers: Google Docs (Drive API), Calendar (deep event history)
- Data stored outside vault (default `~/.kb/`): Parquet + LanceDB + Tantivy + SQLite state
