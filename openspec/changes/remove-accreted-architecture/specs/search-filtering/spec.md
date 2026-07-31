## MODIFIED Requirements

### Requirement: Vector search applies filters before limit
The system SHALL return the requested number of results when filters are applied, by over-fetching candidates before filtering and then truncating to the limit. The over-fetch factor SHALL be documented at the call site.

#### Scenario: Search with source filter and limit of 10
- **WHEN** a user searches with `source=slack` and `limit=10`
- **THEN** up to 10 results are returned and every one has `source=slack`

#### Scenario: Search with after-date filter
- **WHEN** a user searches with `after=2026-01-01` and `limit=5`
- **THEN** every returned result has a date on or after 2026-01-01

#### Scenario: Filter excludes more than the over-fetch window
- **WHEN** filters exclude more candidates than the over-fetch provides
- **THEN** fewer than `limit` results are returned rather than unfiltered results being padded in

### Requirement: One search entry point serves every caller
The daemon, the CLI, and the MCP server SHALL call the same search function, so filtering behaviour cannot diverge between surfaces.

#### Scenario: MCP search matches daemon search
- **WHEN** the same query with the same filters is executed via the MCP tool and via the daemon's `Command::Search`
- **THEN** both return identical results

#### Scenario: No duplicate search implementation exists
- **WHEN** the workspace is searched for hybrid-search call sites
- **THEN** every caller routes through the shared entry point, and no crate carries its own copy of the over-fetch-and-filter logic
