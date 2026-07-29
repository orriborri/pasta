## Context

Available data sources:
- **Slack**: `conversations.list`, `conversations.history` (paginated, Tier 3 rate limit ~50 req/min)
- **Gmail**: `gog gmail search --all --json` with pagination
- **Linear**: GraphQL API via `linear-api`, cursor-based pagination
- **Wiki**: 86 markdown pages at `/home/orre/ReadPeak/wiki/`
- **Repos**: Local clones at `/home/orre/ReadPeak/` (mononode, eks-workloads, cdk, intelligence, etc.)

Storage: Obsidian vault + LanceDB embedded index.

## Goals / Non-Goals

**Goals:**
- Bulk sync from Slack, Gmail, Linear, wiki, and key repos into structured markdown
- Index everything into LanceDB for semantic search by AI agents
- Incremental daily updates (only new/changed content)
- Agents can query: "what did I discuss with X about Y?", "how does our pipeline work?", "what was decided about Z?"

**Non-Goals:**
- Real-time sync (hourly feeds handle "current" state)
- Full-text search UI in the TUI (agents use the index)
- Syncing binary files or attachments

## Decisions

### 1. Two-layer storage: markdown + LanceDB

```
.history/
├── slack/dm/<person>.md
├── slack/channels/<channel>.md
├── gmail/<YYYY-MM>/<subject-slug>.md
├── linear/<identifier>.md
├── wiki/<page-name>.md           ← copied/symlinked from ReadPeak/wiki
├── repos/<repo>/<file-path>.md   ← key source files indexed
└── _sync_state.json

.lancedb/                          ← LanceDB embedded database
└── (managed by LanceDB)
```

Markdown for human-readability and grep. LanceDB for semantic retrieval.

### 2. LanceDB embedded in backend

Use the `lancedb` Rust crate directly in the backend binary. Schema:

```
Table: chunks
- id: String (uuid)
- source: String (slack|gmail|linear|wiki|repo)
- path: String (file path)
- participants: String[] 
- date: String
- content: String (chunk text)
- vector: Vector[1536] (embedding)
```

### 3. Embedding strategy

Use OpenAI `text-embedding-3-small` (1536 dims, cheap, fast). Chunk content into ~500 token segments with overlap. Fall back to local model (nomic-embed-text via Ollama) if API unavailable.

### 4. What to index from repos

Not all source code — focus on:
- README files
- Documentation (docs/, doc/)
- Configuration files (Terraform, CDK stacks, CI pipelines)
- Key architecture files

Configurable via `.history/repo-index.toml`:
```toml
[[repos]]
path = "/home/orre/ReadPeak/mononode"
include = ["README.md", "docs/**", "apps/platform/graphql/src/**/*.ts"]

[[repos]]
path = "/home/orre/ReadPeak/eks-workloads"
include = ["**/*.ts", "README.md"]
```

### 5. Wiki: copy on sync

Copy wiki markdown files into `.history/wiki/` during sync. They're already well-structured documentation — just chunk and embed them.

### 6. Sync schedule

- **Heavy sources** (Slack DMs, Gmail): daily, incremental
- **Light sources** (wiki, repos): daily, full re-index (fast since local files)
- **Linear**: daily, incremental (by updatedAt)
- Add as a native schedule in backend (separate from hourly feed fetchers)

### 7. Agent query interface

Agents query by calling a CLI tool or through an MCP-like interface:
```bash
pasta-search "deployment strategy for eks" --source wiki,slack --limit 10
```

Returns ranked chunks with source, date, participants. The backend exposes this via IPC command so TUI-connected agents can use it too.

## Risks / Trade-offs

- **First sync takes time** → Progress tracking in `_sync_state.json`, resumable
- **OpenAI API cost** → ~$0.02 per 1M tokens for embedding. Full history ~5-10M tokens = ~$0.10-0.20 one-time
- **LanceDB binary size** → Adds to backend binary, but it's embedded so no extra process
- **Stale repo index** → Re-index daily is fine; code doesn't change that fast
- **Rate limits** → Slack throttled at 50 req/min with built-in delays
