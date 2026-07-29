## Why

The codebase has accumulated dead code from iterative refactoring (standalone `fetch()` functions, old embedding helpers, unused JSON-based vector store) and several behavioral issues that degrade search quality and reliability. Cleaning this up reduces compile warnings, shrinks binary size, and fixes user-facing bugs in search and history parsing.

## What Changes

- **Remove dead code**: standalone `fetch()` functions in all fetchers, `embed_openai`/`embed_ollama` standalone functions, `SIMILARITY_THRESHOLD` constant, `SpawnDecision.deps_to_clear` field, entire `simple_index.rs` module
- **Fix MCP search post-limit filtering**: apply source/participant/after filters in LanceDB query (pre-limit) instead of post-retrieval
- **Fix git log parsing**: use null-byte record separator properly instead of splitting on `\n\n`
- **Fix repair_links search quality**: skip frontmatter when building the search query for vault documents
- **Fix embedding dimension mismatch**: normalize Ollama vectors to unit length before zero-padding, or use separate distance function awareness

## Capabilities

### New Capabilities
- `search-filtering`: Pre-limit filtering in vector search to ensure relevant results aren't excluded

### Modified Capabilities
- `state-persistence`: Fix git log parsing and repair_links search quality (both affect indexed content quality)

## Impact

- `crates/backend/src/fetchers/{gitlab,gmail,linear,slack}.rs` — remove `fetch()` functions
- `crates/backend/src/history/embedder.rs` — remove dead `embed_openai`/`embed_ollama`
- `crates/backend/src/history/simple_index.rs` — delete entire module
- `crates/backend/src/history/mod.rs` — remove `simple_index` module declaration
- `crates/backend/src/history/vault_organize.rs` — remove `SIMILARITY_THRESHOLD`, fix `repair_links` search query
- `crates/backend/src/scheduler.rs` — remove `deps_to_clear` from `SpawnDecision`
- `crates/backend/src/history/sync_git_log.rs` — fix log parsing
- `crates/mcp/src/main.rs` — apply search filters before limit
- `crates/backend/src/history/indexer.rs` — apply search filters before limit (shared pattern)
