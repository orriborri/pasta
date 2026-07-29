## Context

Pasta is a Rust daemon that syncs data from Slack, Gmail, Linear, GitLab wiki, and git repos into `.history/`, indexes them into a vector store (LanceDB + JSON fallback), and serves this via a Unix socket to the TUI. The `pasta-search` binary provides CLI search. AI agents currently access data only through `.feeds/` file snapshots or the vault filesystem.

## Goals / Non-Goals

**Goals:**
- Expose pasta's indexed data as MCP tools over stdio transport
- Single binary: `pasta-backend --mcp` starts the MCP server
- Tools are read-only (no mutations via MCP)
- Works with any MCP client: Kiro CLI, Claude Desktop, Cursor
- Subsumes the SDLC Context MCP Server project

**Non-Goals:**
- SSE/HTTP transport (stdio is sufficient for local use)
- Write operations (creating tasks, sending messages)
- Running the scheduler alongside MCP mode (separate concerns)
- Authentication (local stdio = trusted)

## Decisions

### 1. New crate `crates/mcp`

Keeps MCP concerns separate from the scheduler/TUI logic. Depends on `pasta-common` for vault/data types and imports search functions from `crates/backend` (or extracts the search into `crates/common`).

### 2. Tool definitions

```rust
#[tool(description = "Semantic search over all indexed history (Slack, Gmail, Linear, wiki, code)")]
async fn search_history(query: String, source: Option<String>, limit: Option<usize>) -> CallToolResult

#[tool(description = "List vault tasks filtered by status, priority, or project")]
async fn search_tasks(status: Option<String>, priority: Option<u8>, project: Option<String>) -> CallToolResult

#[tool(description = "Get current feed state from all sources")]
async fn get_feeds() -> CallToolResult

#[tool(description = "Get people with their identifiers and items you're waiting on from them")]
async fn get_people(name: Option<String>) -> CallToolResult

#[tool(description = "Get today's daily note content")]
async fn get_daily() -> CallToolResult
```

### 3. Entry point

In `crates/backend/src/main.rs`, add:
```rust
if std::env::args().any(|a| a == "--mcp") {
    return mcp::serve_stdio().await;
}
```

Or make `crates/mcp` its own binary (`pasta-mcp`) for cleaner separation.

**Decision:** Separate binary `pasta-mcp` — simpler, no interaction with scheduler, can be killed independently.

### 4. Search implementation

Reuse `simple_index::search()` directly. It loads the JSON vector store, embeds the query, and returns ranked results. For MCP this is fine — the store is <50MB, loads in <1s.

### 5. MCP config for Kiro

Add to `.kiro/settings/mcp.json`:
```json
{
  "pasta": {
    "command": "/home/orre/Obsidian/Readpeak/pasta/target/release/pasta-mcp",
    "type": "stdio",
    "disabled": false
  }
}
```

### 6. Response formatting

Return structured text (not raw JSON) from tools — keeps context windows efficient. Example for `search_history`:
```
Found 5 results for "EKS upgrade":

1. [slack] 2026-06-10 | #devops | Orre, Wille
   Discussed K8s 1.33 upgrade timeline, agreed to target Q3...

2. [linear] 2026-06-11 | OPS-179
   Setup package deployment support — blocked by EKS version...
```

## Risks / Trade-offs

- **[Trade-off] Cold start** — loading the vector store on each MCP invocation adds ~1s. Acceptable for tool calls.
- **[Risk] Stale data** — MCP server reads the last-synced state. Not real-time, but good enough (syncs run hourly).
- **[Trade-off] Separate binary vs integrated** — separate is simpler but means another process. Since stdio MCP servers are spawned on-demand by the client, this is fine.
