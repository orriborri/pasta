## Context

Pasta is a personal daemon (~5k LoC) that grew a knowledge base engine (~15k LoC) inside it. The KB now needs a real pipeline (pre-processing, entity extraction, multi-modal search) which doesn't belong in a daemon's maintenance loop. We're splitting it out.

## Goals / Non-Goals

**Goals:**
- Standalone KB engine that can run as CLI, library, or server
- Proper pre-processing pipeline between raw fetch and indexing
- Arrow/Parquet raw storage for efficient querying and replay
- Three search modes: vector (semantic), full-text (keyword), structured (SQL-like)
- Incremental sync with state management
- MCP server for AI tool access
- Deployable locally or on AWS (ECS Fargate)

**Non-Goals:**
- Real-time streaming (batch/scheduled is fine)
- Multi-tenant access control (single-user for now, team later)
- Web UI (CLI + MCP + TUI is enough)
- Replacing pasta's agent orchestration or task management

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        kb-engine                                   │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    FETCHERS (parallel via tokio::join!)       │ │
│  │  Gmail │ Slack │ Linear │ Git │ Google Docs │ Calendar        │ │
│  │    ↓       ↓       ↓      ↓        ↓           ↓            │ │
│  │           stream::select_all (merge into one stream)          │ │
│  └────────────────────────┬────────────────────────────────────┘ │
│                           │ Stream<RecordBatch>                   │
│                           │ (backpressure via bounded channels)   │
│                           ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              PIPELINE (tokio streams, batched)                │ │
│  │                                                               │ │
│  │  .chunks(100)                                                 │ │
│  │    → NormalizeStage     (resolve IDs via People/)             │ │
│  │    → FilterStage        (skip bots, CI noise)                 │ │
│  │    → DeduplicateStage   (content hash)                        │ │
│  │    → ExtractStage       (regex: AD-365, !2935, emails)        │ │
│  │    → SummarizeStage     (LLM or heuristic for long threads)   │ │
│  │    → EnrichStage        (link entities via Projects/, Tasks/) │ │
│  │  .buffer_unordered(4)                                         │ │
│  │    → EmbedStage         (OpenAI/Ollama batch embed)           │ │
│  └────────────────────────┬────────────────────────────────────┘ │
│                           │ enriched + embedded batches           │
│                           ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    STORAGE (Parquet authoritative)            │ │
│  │                                                               │ │
│  │  → ParquetStore   (SOURCE OF TRUTH — append/replace by ID)    │ │
│  │  → VectorStore    (LanceDB — derived, rebuildable)            │ │
│  │  → TextIndex      (Tantivy — derived, rebuildable)            │ │
│  │  → EntityStore    (work_items.parquet cross-references)       │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    ENTITY REGISTRY (shared, in-memory)        │ │
│  │                                                               │ │
│  │  Loaded from:                                                 │ │
│  │    People/          → identities (slack_id, email, gitlab)    │ │
│  │    1. Projects/     → repos, channels, linear prefixes        │ │
│  │    Tasks/           → initiative subfolder mapping             │ │
│  │                                                               │ │
│  │  Auto-extends vault files when new entities discovered        │ │
│  │  User edits always win (never overwrite)                      │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    QUERY LAYER                                │ │
│  │                                                               │ │
│  │  search("auth migration") →                                   │ │
│  │    1. Vector search (LanceDB) → semantic matches              │ │
│  │    2. Full-text (Tantivy) → keyword matches                   │ │
│  │    3. RRF merge + dedupe                                      │ │
│  │    4. Enrich with entity cross-references                     │ │
│  │                                                               │ │
│  │  Interfaces: MCP server │ CLI │ HTTP API │ Library            │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    SYNC STATE (SQLite)                        │ │
│  │  cursors, last-sync timestamps, content hashes                │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

## Project Structure

kb-engine lives in the existing `pasta/` workspace as new `kb-*` crates — no new repo, no code move. Pasta and kb-engine share the `common` crate and one build.

```
pasta/                            (existing repo — umbrella monorepo)
├── Cargo.toml                    (one workspace; add kb-* to members)
├── crates/
│   ├── common/                   (shared: Record types, config, vault access)
│   ├── backend/                  (pasta daemon — existing)
│   ├── tui/                      (pasta TUI — existing)
│   ├── mcp/                      (rewired to kb-search)
│   │
│   ├── kb-core/                  (Record struct, config, sync state, ID gen)
│   ├── kb-fetchers/              (gmail, slack, linear, git, gdocs, calendar)
│   ├── kb-pipeline/              (normalize, filter, dedupe, extract, summarize, enrich)
│   ├── kb-storage/               (parquet_store, vector_store, text_index)
│   ├── kb-search/                (hybrid_search, ranking)
│   └── kb-cli/                   (kb binary: sync, search, stats, reindex)
└── openspec/

Binaries produced: pasta-backend, pasta (tui), kb, kb-mcp

Data (outside the vault, configurable — default ~/.kb/):
~/.kb/
├── raw/                          (parquet files — source of truth)
├── vectors/                      (lancedb — derived)
├── index/                        (tantivy — derived)
└── state.db                      (sqlite sync state)
```

Pasta calls kb-search as an in-process library (same workspace) — no subprocess overhead. The `kb` CLI and `kb-mcp` server are thin wrappers over the same crates for standalone/external use.

## Decisions

### 1. Arrow/Parquet as the raw data layer

**Decision**: Store all ingested data as Parquet files, partitioned by source and month.

**Rationale**: 
- Replay pipeline from raw data without re-fetching
- Efficient columnar queries (DataFusion SQL)
- Standard format — can be read by any tool (Python, DuckDB, etc.)
- Much smaller than markdown files (compressed, columnar)

**Schema** (universal across sources):
```
id: Utf8
source: Utf8 (gmail|slack|linear|git|gdocs|calendar)
kind: Utf8 (message|thread|issue|commit|doc|event)
title: Utf8
content: Utf8
author: Utf8
participants: List<Utf8>
created_at: Timestamp
updated_at: Timestamp
url: Utf8
thread_id: Utf8 (for grouping conversations)
entities: List<Utf8> (extracted: people, projects, MRs)
tags: List<Utf8>
```

### 2. Pipeline operates on Record structs, not Arrow batches

**Decision**: The pipeline processes plain `Record` structs (row-wise). Arrow/Parquet is used ONLY at the storage boundary, not between pipeline stages.

```rust
trait PipelineStage: Send + Sync {
    async fn process(&self, records: Vec<Record>) -> Result<Vec<Record>>;
    fn name(&self) -> &str;
}

struct Pipeline {
    registry: Arc<EntityRegistry>,
    stages: Vec<Box<dyn PipelineStage>>,
}
```

**Rationale**: Pipeline stages are inherently row-wise — regex extraction, LLM summarization, entity resolution all operate on a single record's content. Forcing these through Arrow `RecordBatch` (columnar) means constant columnar↔row conversion, killing the zero-copy benefit. Arrow's strength is analytical queries over *stored* data, not row-wise text transforms.

**Where Arrow IS used**:
- `ParquetStore` converts `Vec<Record>` → Arrow `RecordBatch` once, at write time
- DataFusion reads Parquet (Arrow-native) for SQL queries
- Nowhere else in the pipeline

**Execution model** (tokio streams over Records):

```rust
impl Pipeline {
    async fn run(&self, sources: Vec<Box<dyn Source>>) -> Result<()> {
        let streams = sources.iter().map(|s| s.stream());
        let input = stream::select_all(streams);     // Stream<Record>

        let mut current = input.chunks(50).boxed();  // Vec<Record> for batch efficiency
        for stage in &self.stages {
            current = current.then(|recs| stage.process(recs)).boxed();
        }

        // Terminal: Parquet is source of truth, LanceDB+Tantivy derived from it
        current
            .then(|recs| self.storage.write(recs))
            .for_each(|_| async {})
            .await;
        Ok(())
    }
}
```

**Why not Akka Streams / Scala**: 
- JVM adds 200MB+ base memory, second language in stack
- Tokio streams give same model: backpressure, composition, fan-out/in
- Same language as LanceDB, Tantivy, Parquet integration

**Pipeline properties**:
- Backpressure: bounded channels + stream buffering prevent OOM
- Batching: `.chunks(50)` groups records for batched LLM/embed calls
- Parallelism: `tokio::join!` for fan-out fetch; `buffer_unordered` for concurrent embed in storage
- Composable: stages are `Vec<Box<dyn PipelineStage>>`, reorderable via config
- Simple: plain structs, Arrow conversion only at storage

### 3. Hybrid search with result fusion

**Decision**: Query layer runs vector + full-text in parallel, merges results using Reciprocal Rank Fusion (RRF).

**Rationale**: Vector search finds semantically related content; full-text finds exact mentions. RRF is simple, effective, and parameter-free.

### 4. SQLite for sync state (local), DynamoDB-compatible for AWS

**Decision**: Use SQLite locally for sync cursors, content hashes, job history. Schema is simple enough to port to DynamoDB if deployed on AWS.

### 4b. Parquet is the source of truth; LanceDB + Tantivy are derived

**Decision**: Parquet files are authoritative. LanceDB (vectors) and Tantivy (full-text) are *derived indexes*, fully rebuildable from Parquet via `kb reindex`.

**Rationale**: Three stores can't share a transaction. Making two of them derived removes the consistency problem — if LanceDB/Tantivy drift or corrupt, rebuild from Parquet. Writes go Parquet-first; if a derived-index write fails, it's logged and fixed on next reindex, not a data-loss event.

**Write order**:
1. Append/replace records in Parquet (authoritative)
2. Upsert vectors in LanceDB (derived)
3. Update Tantivy index (derived)

If step 2 or 3 fails, the record is still safe in Parquet and will be re-indexed.

### 4c. Update semantics — stable IDs, replace-by-ID

**Decision**: Every record has a stable, deterministic ID (`<source>-<native_id>`). Re-syncing a changed item (new Slack reply, Linear status change) replaces the record by ID in all three stores.

**Rationale**: Incremental sync isn't append-only — threads grow, issues change status. Without stable IDs we'd get duplicates. The ID is computed from source + native identifier, so the same item always maps to the same record.

```
Slack message:  slack-<channel>-<ts>
Slack thread:   slack-thread-<channel>-<root_ts>
Gmail thread:   gmail-<thread_id>
Linear issue:   linear-<identifier>      (e.g. linear-AD-365)
GitLab MR:      gitlab-<project>-<iid>   (e.g. gitlab-mononode-2935)
Git commit:     git-<repo>-<sha>
Calendar event: gcal-<event_id>
Task:           task-<file_stem>
```

On re-sync: content hash compared; if changed, replace record (Parquet row, LanceDB vector, Tantivy doc) by ID. If unchanged, skip (no re-embed).

### 4d. Single embedding model, locked at init

**Decision**: The embedding model is chosen once and recorded in sync state. The system refuses to mix models in one LanceDB table. Changing models requires a full `kb reindex`.

**Rationale**: This is a fresh build — fix the pasta dimension-mismatch bug now. Mixed 1536-dim (OpenAI) and zero-padded 768-dim (Ollama) vectors produce meaningless similarity scores. Lock the model; if it changes, rebuild (cheap since Parquet is source of truth).

### 5. LLM summarization as optional stage

**Decision**: Thread summarization calls an LLM (via configured endpoint). Stage is optional — can be disabled for cost or privacy reasons. Falls back to "first message + last message" heuristic.

### 5b. Linking rules derived from vault files (zero config)

**Decision**: All cross-source linking rules are derived from vault files — no separate config file needed:

- **People/** → identity resolution (Slack ID, GitLab user, email → canonical person)
- **1. Projects/** → project mapping (gitlab_repos, linear_prefix, slack_channels → canonical project)
- **Work items** → auto-discovered via regex patterns in content (e.g., `AD-365`, `!2935`)

Project files gain optional frontmatter fields that the pipeline reads:
```yaml
---
gitlab_repos: ["readpeak/mononode"]    # messages mentioning this repo → this project
linear_prefix: AD                       # AD-* issues → this project
slack_channels: ["#ad-delivery"]        # messages in this channel → this project
---
```

If these fields are missing, pipeline discovers them (from Linear API project data, GitLab group structure, etc.) and auto-adds them — same "only add, never overwrite" rule as People/.

Link extraction uses simple regex:
- `[A-Z]{2,5}-\d+` → Linear issue
- `!(\d{3,5})` → GitLab MR
- `/merge_requests/(\d+)` → GitLab MR
- Branch name pattern `<prefix>-\d+-*` → Linear issue

All linking is pattern matching + vault file lookups. No ML required.

### 6. Entity resolution via People/ directory

**Decision**: Use `People/` vault files as the entity registry. The pipeline:
- Loads all People/ files on startup as the identity lookup table
- Auto-creates new `People/<Name>.md` ONLY for internal contacts (matches an allowlist: `@readpeak.com` emails, members of the readpeak GitLab group, the Slack workspace). External senders (customers, mailing lists, one-off contacts) stay as raw author strings and are NOT promoted to People/ files.
- Auto-adds missing identifiers to existing files (e.g., discovers GitLab username for a known person)
- Never overwrites fields the user manually edited (checks mtime vs discovered timestamp)
- Never touches markdown body below the frontmatter (user notes are safe)
- Marks confidence: low/medium/high based on how the link was discovered
- Surfaces low-confidence discoveries in the daily note for user review

Same pattern extends to `1. Projects/` files for project↔repo↔channel mapping.

**Frontmatter contract**:
```yaml
---
name: Ville Koskinen
gitlab: ville-koskinen          # pipeline-discovered
slack_id: U09XYZ123             # pipeline-discovered
email: ville@readpeak.com       # pipeline-discovered
role: Developer                 # user-added (pipeline won't touch)
team: Platform                  # user-added (pipeline won't touch)
discovered: 2026-06-25          # when first seen
last_seen: 2026-06-25           # updated each sync
confidence: high                # low → medium → high as IDs confirmed
---
```

**Rules**:
- User edits always win (field mtime > discovered → skip)
- Pipeline only ADDS new fields, never removes or changes existing
- Body content below `---` is never read or written by pipeline

## Crate Dependencies

```
core:       serde, chrono, arrow-schema, anyhow, dirs
fetchers:   reqwest, tokio, serde_json, core
pipeline:   arrow-array, core, (optional: reqwest for LLM calls)
storage:    parquet, lancedb, tantivy, datafusion, arrow, core
search:     storage, core
mcp:        rmcp, search, core
cli:        clap, search, storage, fetchers, pipeline, core
```

## How pasta integrates

```toml
# pasta.toml
[kb]
engine = "kb-engine"           # path to binary or "library"
data_dir = "~/.kb"
sync_on_fetch = true           # run kb-engine sync after fetch cycle

[schedules.kb-sync]
interval_minutes = 60
enabled = true
```

Pasta calls: `kb-engine sync --incremental`
Pasta calls: `kb-engine search "query" --limit 10 --json`

Or links kb-engine as a library crate for in-process search.

## Migration Path

1. **Phase 1 (KILL GATE)**: Walking skeleton — ONE source (Slack) → Record → Parquet → LanceDB → search. Compare against current pasta on 10 test queries. If not better/equal-with-headroom, STOP.
2. **Phase 2**: Tantivy + hybrid search, sync state, update-by-ID, reindex, MCP.
3. **Phase 3**: Remaining fetchers (Gmail, Linear, Git, Tasks, Calendar, Google Docs).
4. **Phase 4**: Pipeline stages (normalize, filter, dedupe-exact, extract, dedupe-cross, summarize).
5. **Phase 5**: Entity registry + enrich + work-item linking.
6. **Phase 6**: Pasta integration, remove history/ from pasta.
7. **Phase 7** (optional): HTTP API + AWS deploy, only if team use materializes.

Pasta and kb-engine coexist during transition.

## Task Creation Rules (configurable)

Pasta's inbox-processor uses configurable rules to decide what to do with new items from kb-engine:

```
kb-engine record → match against rules → auto_create | ask | skip
```

Three actions:
- **auto_create**: Task file created in `Tasks/` (untagged → Kanban "Uncategorized")
- **ask**: Listed in daily note with "create task?" checkbox — user decides
- **skip**: Not surfaced at all

Rules are TOML config with `source`, `signal` label, and `match` filter:

```toml
[[task_rules.auto_create]]
source = "gitlab"
signal = "review_requested"
match = "assignee:me OR reviewer:me"

[[task_rules.auto_create]]
source = "linear"
signal = "assigned"
match = "assignee:me"

[[task_rules.auto_create]]
source = "slack"
signal = "dm_question"
match = "is:dm AND has:question"

[[task_rules.auto_create]]
source = "calendar"
signal = "needs_rsvp"
match = "response_status:needsAction"

[[task_rules.ask]]
source = "slack"
signal = "channel_mention"
match = "mentions:me AND is:channel"

[[task_rules.ask]]
source = "gmail"
signal = "notification"
match = "NOT label:bot AND NOT label:Gitlab"

[[task_rules.skip]]
source = "*"
signal = "bot"
match = "author:*-bot OR author:gitlab-bot"

[[task_rules.skip]]
source = "gmail"
signal = "promotional"
match = "label:promotions OR label:social"
```

Rules are evaluated top-to-bottom, first match wins. Users add/edit rules without code changes. The match syntax supports field:value with AND/OR/NOT and wildcards.

## Deployment Options

| Mode | How | Cost |
|------|-----|------|
| Local (laptop) | `kb-engine sync && kb-engine serve` | $0 |
| Local + pasta | pasta schedules kb-engine sync | $0 |
| AWS MVP | ECS Fargate + S3 + EFS | ~$50/mo |
| AWS team | + OpenSearch + DynamoDB + ALB | ~$150-300/mo |

## Risks / Trade-offs

- [Scope is large — 6+ phases] → Phase 1 is a kill gate. Build one source end-to-end, measure against current pasta. Don't build on faith.
- [Migration complexity] → Phase it. Pasta and kb-engine coexist during transition. Both can read LanceDB.
- [Three stores drift] → Parquet is source of truth; LanceDB + Tantivy are derived and rebuildable via `kb reindex`. No cross-store transaction needed.
- [Embedding dimension mismatch (pasta's bug)] → Single model locked at init, recorded in sync state, mixing refused. Model change requires reindex (cheap — Parquet is truth).
- [Incremental update vs duplicates] → Stable deterministic IDs; replace-by-ID; skip unchanged via content hash.
- [People/ file explosion] → Auto-create only for internal contacts (allowlist). External senders stay as raw strings.
- [LLM costs for summarization] → Optional stage. Heuristic fallback is free.
- [Google OAuth/Docs complexity] → Reuse `gog` auth (already used for Gmail). Treated as its own sub-effort, not a one-line fetcher.
- [Arrow/row-wise mismatch] → Pipeline uses plain Record structs; Arrow only at Parquet write boundary.
