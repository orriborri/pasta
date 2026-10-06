# kb-engine

Personal knowledge base engine. Ingests data from Slack, Gmail, Linear, Git, Calendar, Google Docs, and vault files into a searchable hybrid index (semantic + full-text).

> **Pasta integration:** pasta no longer maintains its own `history/` index. The
> daemon delegates all search to kb-engine (`kb_storage::hybrid_search`) and all
> ingestion to the `kb-sync` schedule (`kb sync`). kb-engine's `~/.kb` store is
> the single index. The old `--migrate`/`--reindex`/`--backfill`/`--sync-once`
> pasta CLI flags are removed; use `kb sync` / `kb reindex` instead.

## Quick Start

```bash
# Build
cargo build -p kb-cli -p kb-mcp

# First sync (fetches last 30 days from all sources)
RUST_LOG=info kb sync

# Search
kb search "deployment rollback"
kb search "double verify"

# Sync only specific sources
kb sync --source slack,linear

# Rebuild indexes after model change
kb reindex
```

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────────┐
│  Fetchers   │ ──▶ │   Pipeline   │ ──▶ │    Storage       │
│  slack      │     │  normalize   │     │  Parquet (truth) │
│  gmail      │     │  filter      │     │  LanceDB (vec)   │
│  linear     │     │  dedupe      │     │  Tantivy (text)  │
│  git        │     │  extract     │     │  SQLite (state)  │
│  vault      │     │  enrich      │     └─────────────────┘
└─────────────┘     └──────────────┘              │
                                           ┌──────┴──────┐
                                           │  Hybrid     │
                                           │  Search     │
                                           │  (RRF)      │
                                           └──────┬──────┘
                                                  │
                                    ┌─────────────┼─────────────┐
                                    │             │             │
                                 kb CLI      kb-mcp       pasta-mcp
```

## Data Layout

```
~/.kb/
├── raw/              Parquet files (source of truth, partitioned by source/month)
│   └── slack/2026-06/20260626143512.parquet
├── vectors/          LanceDB (derived, rebuildable)
├── index/            Tantivy (derived, rebuildable)
└── state.db          SQLite (cursors, content hashes, model lock)
```

## Configuration

In `~/.pasta/config.toml`:

```toml
[kb]
data_dir = "~/.kb"

# Auto-create People/ files for contacts matching these
internal_domains = ["@readpeak.com"]
internal_slack = true
```

## Commands

| Command | Description |
|---------|-------------|
| `kb sync` | Fetch all sources, run pipeline, store |
| `kb sync --source slack` | Sync only specific sources |
| `kb search "query"` | Hybrid search (vector + full-text) |
| `kb recent --days N` | List records from the last N days as JSON (pasta inbox processing) |
| `kb changes --json [--cursor CURSOR]` | Page through changed records for incremental external consumers |
| `kb reindex` | Rebuild LanceDB + Tantivy from Parquet |

## Crates

| Crate | Purpose |
|-------|---------|
| `kb-core` | Record struct, config, sync state (SQLite) |
| `kb-storage` | Parquet, LanceDB, Tantivy, embedder, hybrid search |
| `kb-pipeline` | Pipeline stages: normalize, filter, dedupe, extract, enrich |
| `kb-fetchers` | Source fetchers: Slack, Gmail, Linear, Git, Vault |
| `kb-cli` | CLI binary (`kb`) |
| `kb-mcp` | MCP stdio server (`kb-mcp`) |

## Key Design Decisions

- **Parquet is source of truth** — LanceDB and Tantivy are derived, rebuildable via `kb reindex`
- **Single embedding model** — locked at init, stored in state.db, refuses mixing. Change via `kb reindex`
- **Pipeline processes plain structs** — Arrow conversion only at Parquet write boundary
- **Incremental sync** — per-channel cursors in SQLite, content hashes skip unchanged records
- **Hybrid search** — vector + full-text merged via Reciprocal Rank Fusion (k=60)

## Prerequisites

- `slack-api` binary on PATH (or `SLACK_API_PATH` env)
- `gog` binary for Gmail (or `GOG_PATH` env)
- `linear-api` binary for Linear (or `LINEAR_API_PATH` env)
- OpenAI API key at `~/.config/openai/api_key` (or `OPENAI_API_KEY` env) — falls back to local Ollama

## Evidence reliability and migration

Fetched records are atomically saved to Parquet before source cursors commit.
The raw journal preserves full bodies and metadata; normalization and entity
extraction build one derived record per source ID without thread merging or
heuristic truncation. Index completion hashes are written only after text,
vector, and graph writes succeed. Every sync revisits unfinished durable records,
even if the source returns no new data. The scheduled daemon also syncs the vault.

`read_all` exposes snapshot history, `read_latest` resolves the most recent
snapshot of each ID, and `read_current` additionally retires legacy vault IDs
when their files are captured under relative-path IDs. Existing legacy IDs remain
resolvable as historical evidence; active search indexes remove superseded IDs.
Metadata changes invalidate completion hashes. Graph edges are replaced per
updated evidence record, so removed mentions do not remain current relations.

The change-feed cursor now uses ingestion time (`v2|...`), not source timestamps.
Existing v1 cursors trigger a one-time replay, preventing missed late arrivals.
`get_evidence` retains `snippet` and adds complete `content`, `updated_at`, and a
`version_hash` for inspecting the resolved snapshot. File deletions and remote
service edits are only reflected when their fetcher reports them; this change
does not introduce source tombstones or webhook ingestion.

After upgrading, run `kb sync` and optionally `kb reindex` to rebuild indexes.
Previously discarded source text cannot be recovered from old Parquet snapshots;
refetch the relevant source history where available. Gmail ingestion still stores
the snippets returned by its fetcher, rather than fetching complete email bodies.
The lossless guarantee starts at the records supplied by each fetcher.

Atomic Parquet writes and wiki durability checks target local POSIX filesystems.
Keep maintenance to one daemon/CLI writer at a time; scheduling single-flight
protection does not coordinate separate CLI processes.
