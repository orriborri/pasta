## MODIFIED Requirements

### Requirement: MCP server exposes knowledge base tools
The system SHALL provide exactly one MCP stdio server, `kb-mcp`, exposing the `search_knowledge` tool. A second MCP server duplicating knowledge-base search SHALL NOT exist in the workspace.

#### Scenario: AI tool searches knowledge base
- **WHEN** an MCP client calls `search_knowledge` with query "EKS upgrade process"
- **THEN** it returns hybrid search results with source links, snippets, and relevance scores

#### Scenario: Only one search implementation is shipped
- **WHEN** the workspace is built
- **THEN** `kb-mcp` is the only MCP binary exposing knowledge-base search, and it shares the search entry point used by the daemon

### Requirement: CLI interface for sync and search
The system SHALL provide the CLI commands that exist: `kb sync [--source <name>]`, `kb search`, `kb recent`, `kb reindex`, `kb reprocess`, and `kb serve`.

#### Scenario: Manual sync
- **WHEN** user runs `kb sync --source slack`
- **THEN** Slack records are fetched, processed, and indexed

#### Scenario: CLI search
- **WHEN** user runs `kb search "deployment rollback procedure" --limit 5`
- **THEN** top 5 hybrid search results are printed with source, date, and snippet

### Requirement: Pasta integration via library only
Pasta SHALL consume kb-engine as a Rust library crate. The daemon SHALL NOT spawn `kb` as a subprocess, so a missing or unbuilt `kb` binary cannot disable daemon functionality.

#### Scenario: Daemon syncs the knowledge base
- **WHEN** the user triggers a backfill from the TUI
- **THEN** the daemon calls `kb_sync` in-process and reports the real outcome, succeeding or failing visibly

#### Scenario: kb binary is not installed
- **WHEN** no `kb` executable exists on `PATH`
- **THEN** every daemon feature — fetch, index, search, inbox processing — still works

#### Scenario: Inbox processing reads recent records
- **WHEN** inbox processing runs
- **THEN** it reads recent records through the kb library rather than parsing the stdout of a subprocess

## ADDED Requirements

### Requirement: Data directory resolves once for readers and writers
The system SHALL resolve `[kb] data_dir` in exactly one place, and every reader and writer SHALL obtain its configuration from that resolution rather than constructing a default.

#### Scenario: Non-default data directory configured
- **WHEN** `[kb] data_dir` is set to a path other than `~/.kb`
- **THEN** ingestion writes to that directory and search reads from that same directory

#### Scenario: Data directory left unset
- **WHEN** `[kb] data_dir` is absent from config
- **THEN** all components use the same `~/.kb` default
