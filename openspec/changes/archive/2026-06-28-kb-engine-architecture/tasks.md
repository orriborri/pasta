## Phase 1: Walking Skeleton (ONE source, end-to-end) — KILL GATE

Goal: prove the new architecture beats current pasta search before building all 6 phases.

- [x] 1.1 Add `kb-*` crates to the existing `pasta/` workspace Cargo.toml members (no new repo)
- [x] 1.2 Create `crates/kb-core` — `Record` struct (plain Rust, NOT Arrow), `Source`/`Kind` enums, config, deterministic ID generation
- [x] 1.3 Create `crates/kb-storage` — ParquetStore (Vec<Record> → Arrow → Parquet at write boundary only), VectorStore (LanceDB), single locked embedding model
- [x] 1.4 Port embedder into kb-storage — single model locked at init, model name stored in sync state, refuse mixing
- [x] 1.5 Create `crates/kb-fetchers` with ONE fetcher (Slack) producing `Vec<Record>`
- [x] 1.6 Minimal pipeline: fetch → store (Parquet authoritative → LanceDB derived)
- [x] 1.7 Create `crates/kb-cli` — `kb search` (vector only for now)
- [x] 1.8 **KILL GATE**: Run 10 representative queries against both current pasta and kb-engine. If kb-engine isn't clearly better OR equal-with-headroom, STOP and reassess before Phase 2.

## Phase 2: Storage Completeness + Sync

- [x] 2.1 Add Tantivy TextIndex (derived from Parquet, rebuildable)
- [x] 2.2 Hybrid search — vector + full-text + RRF merge in `crates/search`
- [x] 2.3 Sync state (SQLite) — cursors, content hashes, model name
- [x] 2.4 Update semantics — replace-by-stable-ID across Parquet/LanceDB/Tantivy; skip unchanged (hash match)
- [x] 2.5 `kb reindex` — rebuild LanceDB + Tantivy from Parquet (source of truth)
- [x] 2.6 Port MCP server from pasta, wire to hybrid search

## Phase 3: Remaining Fetchers

- [x] 3.1 Port Gmail fetcher → `Vec<Record>`
- [x] 3.2 Port Linear fetcher → `Vec<Record>`
- [x] 3.3 Port Git log fetcher → `Vec<Record>`
- [x] 3.4 Port Tasks/ + vault folder reader → `Vec<Record>`
- [x] 3.5 Add Calendar fetcher (reuse `gog` if it supports calendar, else gcalcli)
- [x] 3.6 Add Google Docs/Drive fetcher (own sub-effort: OAuth, token refresh, Drive pagination, Docs export) — reuse `gog` auth where possible
- [x] 3.7 Wire all fetchers into `kb sync --source <name>` with `--incremental`

## Phase 4: Pre-processing Pipeline

- [x] 4.1 Define `PipelineStage` trait (`async fn process(Vec<Record>) -> Vec<Record>`)
- [x] 4.2 NormalizeStage — resolve author IDs via People/ lookup
- [x] 4.3 FilterStage — skip bots/CI/promotions (configurable rules)
- [x] 4.4 DedupeStage (exact) — content-hash dedup of identical records, runs early
- [x] 4.5 ExtractStage — regex entity extraction (Linear IDs, MR numbers, emails, URLs)
- [x] 4.6 DedupeStage (cross-source) — merge records that resolve to same work item, runs AFTER extract
- [x] 4.7 SummarizeStage — LLM thread summarization with heuristic fallback (optional)
- [x] 4.8 Pipeline runner — compose stages from config, tokio streams with .chunks(50)

## Phase 5: Entity Registry + Enrich

- [x] 5.1 EntityRegistry — load People/ + Projects/ + Tasks/ subfolders into in-memory lookup
- [x] 5.2 Auto-create People/ for INTERNAL contacts only (allowlist: @readpeak.com, gitlab group, slack workspace)
- [x] 5.3 Auto-add missing identifiers to existing People/ + Projects/ files (add-only, never overwrite user edits)
- [x] 5.4 EnrichStage — link records to work items via Projects/ mappings + extracted entities
- [x] 5.5 Context-aware chunking — keep thread replies together, split on conversation boundaries
- [x] 5.6 work_items.parquet — cross-reference table (Linear ↔ MR ↔ commits ↔ slack ↔ task)
- [x] 5.7 Daily note surfaces low-confidence person/link discoveries for review

## Phase 6: Pasta Integration

- [x] 6.1 Add `[kb]` and `[task_rules]` sections to pasta.toml
- [x] 6.2 Add kb-sync schedule to pasta scheduler (`kb sync --incremental`)
- [x] 6.3 inbox-processor queries kb-engine, applies task_rules (auto_create / ask / skip)
- [x] 6.4 Replace pasta's MCP search with kb-engine MCP
- [ ] 6.5 Remove pasta's history/ modules (keep lightweight feed fetchers if still needed) — deferred to change `history-to-kb-migration`

## Phase 7: Polish + Deploy (optional, only if team use materializes)

- [x] 7.1 `kb serve` HTTP API mode
- [x] 7.2 Dockerfile for ECS Fargate
- [x] 7.3 AWS config: S3 (Parquet), EFS (LanceDB/Tantivy), Secrets Manager (tokens)
- [x] 7.4 Documentation: setup, config reference, architecture
