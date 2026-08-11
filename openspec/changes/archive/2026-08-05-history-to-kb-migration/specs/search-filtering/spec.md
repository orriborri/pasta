## MODIFIED Requirements

### Requirement: MCP and backend search use consistent filtering
The backend `Command::Search` and `vault_manager` semantic-routing search SHALL delegate to kb-engine hybrid search (`kb_storage::hybrid_search`), the same engine the MCP server uses, applying source/participant/date filters pre-limit. Pasta SHALL NOT maintain a separate internal search index.

#### Scenario: MCP search matches backend search
- **WHEN** the same query with filters is executed via the MCP tool and via backend `Command::Search`
- **THEN** both return identical results because both use kb-engine hybrid search

#### Scenario: Backend search after history index removal
- **WHEN** `Command::Search` runs after the `history/indexer` module is removed
- **THEN** it returns results from kb-engine's `~/.kb` store with filters applied pre-limit

> Not promoted to `openspec/specs/search-filtering/spec.md` on archival — see Archival Note in `proposal.md`. `.kiro/specs/remove-accreted-architecture` task 5 owns rewriting that file and will fold in this delta (it removes the same `crates/mcp` reference this delta describes as already gone).
