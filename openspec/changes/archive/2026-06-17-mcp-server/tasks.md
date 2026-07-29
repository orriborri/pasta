## 1. Create `crates/mcp` with rmcp scaffolding

- [x] 1.1 Add `crates/mcp/` to workspace members in root `Cargo.toml`
- [x] 1.2 Create `crates/mcp/Cargo.toml` with deps: `rmcp` (features: transport-io), `pasta-common`, `tokio`, `serde`, `serde_json`, `anyhow`, `tracing`, `schemars`
- [x] 1.3 Create `crates/mcp/src/main.rs` with MCP server struct, `get_info()`, `initialize()`, and stdio serve loop
- [x] 1.4 Verify it compiles and connects via MCP Inspector

## 2. Extract search into a shared library function

- [x] 2.1 Move `simple_index::search()` into `crates/common` (or make it importable from backend)
- [x] 2.2 Ensure the search function works standalone (loads store, embeds query, returns results)
- [x] 2.3 Expose vault read functions from `crates/common` (already there: `load_tasks`, `load_feeds`, `load_people`)

## 3. Implement MCP tools

- [x] 3.1 `search_history` — query + optional source filter + limit, calls vector search, formats results as text
- [x] 3.2 `search_tasks` — filters `load_tasks()` by status/priority/project/person
- [x] 3.3 `get_feeds` — calls `load_feeds()`, formats current state
- [x] 3.4 `get_people` — calls `load_people()`, optionally filters by name
- [x] 3.5 `get_daily` — reads today's daily note file, returns content

## 4. Build and test

- [x] 4.1 Add `pasta-mcp` to the release build in `run-sync.sh` or a new script
- [x] 4.2 Test each tool via MCP Inspector (`npx @modelcontextprotocol/inspector`)
- [x] 4.3 Verify stdio transport works when spawned by a Kiro agent

## 5. Integrate with Kiro

- [x] 5.1 Add `pasta` entry to `.kiro/settings/mcp.json`
- [x] 5.2 Update `personal-assistant` agent to reference pasta MCP tools in its instructions
- [x] 5.3 Test end-to-end: ask an agent "what did I discuss with Wille last week?" and get results from history
