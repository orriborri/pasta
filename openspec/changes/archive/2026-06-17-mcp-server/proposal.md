## Why

Pasta already indexes everything — Slack, Gmail, Linear, wiki, repos, vault — into a searchable vector store. But this knowledge is locked behind the TUI and `pasta-search` CLI. Meanwhile, the SDLC Context MCP Server project in `1. Projects/` proposes building a *separate* Rust MCP server to expose code/wiki/Linear search to AI tools.

These should be the same thing. Pasta *is* the MCP server.

By adding an MCP stdio transport to the pasta-backend binary, any AI client (Kiro CLI, Claude Desktop, Cursor) can directly query your indexed history, vault tasks, people, and feeds. No new codebase, no duplicate indexing.

## What Changes

- Add a new crate `crates/mcp` that implements an MCP server using `rmcp`
- Expose pasta's existing capabilities as MCP tools:
  - `search_history` — semantic search over all indexed content (slack/gmail/linear/wiki/repos)
  - `search_tasks` — find vault tasks by status, priority, project, person
  - `get_feeds` — current state of all feed sources
  - `get_people` — people with their identifiers and waiting items
  - `get_daily` — today's daily note content
- Run as `pasta-backend --mcp` (stdio transport) for use in Kiro/Claude/Cursor configs
- Add to the vault's `.kiro/settings/mcp.json` so agents can use it

## Capabilities

### New Capabilities
- `mcp-tools`: MCP server exposing pasta's search and vault data as tools callable by any MCP client

### Modified Capabilities

## Impact

- New crate: `crates/mcp/` with rmcp dependency
- `pasta-backend --mcp` mode: serves MCP over stdio (no scheduler, no TUI socket)
- Replaces the planned SDLC Context MCP Server project (same goals, simpler path)
- Agents get structured access to *all* pasta data without needing custom skills/scripts
