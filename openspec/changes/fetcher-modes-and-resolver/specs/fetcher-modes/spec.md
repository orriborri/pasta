## ADDED Requirements

### Requirement: Fetchers expose a common two-mode contract
Each external-source fetcher SHALL implement a `Fetcher` trait providing a forward `fetch_updates` mode and, where applicable, on-demand `fetch_targets` and `fetch_backfill` modes. Sources that cannot resolve entities on demand SHALL inherit no-op default implementations.

#### Scenario: Forward update fetch
- **WHEN** the fetch cycle invokes a fetcher's `fetch_updates`
- **THEN** it returns records newer than the source's stored `newest` cursor
- **AND** advances that cursor, preserving today's incremental behavior

#### Scenario: Source without on-demand support
- **WHEN** the resolver asks a fetcher that returns an empty `resolves()` set to fetch targets
- **THEN** the fetcher's default `fetch_targets` returns an empty result without error

### Requirement: Targeted fetch resolves specific entities by reference
A fetcher that declares support via `resolves()` SHALL implement `fetch_targets(&[EntityRef])` to retrieve records for the referenced entities regardless of the forward cursor position or record ownership.

#### Scenario: Linear ticket not assigned to the user
- **WHEN** `fetch_targets` is called with `EntityRef { kind: Linear, id: "AB-123" }`
- **AND** ticket AB-123 is not assigned to the current viewer
- **THEN** the Linear fetcher returns a record for AB-123

#### Scenario: Batched targeted fetch
- **WHEN** `fetch_targets` receives multiple Linear refs sharing a team prefix
- **THEN** the fetcher issues a single batched query rather than one call per ref

### Requirement: Retroactive fetch pages older than the stored oldest cursor
A fetcher SHALL implement `fetch_backfill` to return records older than the source's stored `oldest` cursor, bounded by a caller-supplied budget, and SHALL extend the oldest cursor accordingly.

#### Scenario: Backfill a bounded page
- **WHEN** `fetch_backfill` is called with a budget of one page
- **THEN** it returns records immediately older than the current oldest cursor
- **AND** updates the oldest cursor to the oldest record returned

### Requirement: Entity references are typed and routable
The system SHALL represent references as an `EntityRef` carrying kind, native id, and optional project context, parseable from the entity strings produced by extraction (`linear:AB-123`, `mr:!2935`). A router SHALL dispatch a set of refs to the fetcher whose `resolves()` set covers each kind.

#### Scenario: Route a mixed reference set
- **WHEN** the resolver holds refs of kinds `linear` and `mr`
- **THEN** the router sends `linear` refs to the Linear fetcher and `mr` refs to the GitLab fetcher

#### Scenario: Merge-request ref without project context
- **WHEN** an `mr` ref has no project (a bare `!2935` mention)
- **THEN** the router marks it unresolvable and logs it rather than guessing a project
