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
