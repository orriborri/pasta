## ADDED Requirements

### Requirement: Resolver closes entity references via on-demand fetching
The system SHALL run an asynchronous resolution loop that extracts referenced entities from records, fetches those missing from the knowledge base via targeted mode, re-extracts references from the newly fetched records, and repeats until no new missing references remain.

#### Scenario: Commit references an unindexed ticket
- **WHEN** a fetched commit's content references `AB-123`
- **AND** no record with the deterministic id for `AB-123` exists in the knowledge base
- **THEN** the resolver fetches AB-123 via the Linear fetcher's targeted mode
- **AND** the resulting Linear record is added to the batch before storage

#### Scenario: Reference already present
- **WHEN** a referenced entity already has a record in the knowledge base
- **THEN** the resolver does not refetch it

### Requirement: Resolution is bounded by depth and a fetch budget
The resolver SHALL stop after a configurable maximum hop depth (default 2) or when a per-run fetch budget is exhausted, and SHALL track visited references to avoid refetching within a run.

#### Scenario: Depth limit reached
- **WHEN** resolved records keep introducing new references beyond the maximum depth
- **THEN** the resolver stops at the depth limit and proceeds to storage with what it has

#### Scenario: Resolution disabled
- **WHEN** the maximum depth is configured to 0
- **THEN** the resolver performs no targeted fetches and the batch is stored as fetched

### Requirement: Missing-entity detection uses deterministic record ids
The resolver SHALL determine whether a referenced entity is already known by constructing its deterministic record id and checking the sync state, without performing a search query.

#### Scenario: Existence check by id
- **WHEN** the resolver evaluates reference `linear:AB-123`
- **THEN** it builds id `linear-AB-123` and checks the content-hash store for presence

### Requirement: Resolved records are idempotent across runs
Records produced by targeted fetches SHALL pass through the same content-hash filter as forward-fetched records, so re-resolving an unchanged entity produces no duplicate work.

#### Scenario: Same ticket resolved on two consecutive cycles
- **WHEN** the same unchanged ticket is resolved in two runs
- **THEN** the second run's record is skipped by the hash filter and not re-embedded
