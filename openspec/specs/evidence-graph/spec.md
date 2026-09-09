# evidence-graph Specification

## Purpose
TBD - created by archiving change typed-evidence-graph. Update Purpose after archive.
## Requirements
### Requirement: Entities are represented by a canonical typed reference
The system SHALL represent every graph entity as a canonical `EntityRef` carrying a typed `EntityKind`, a native id, and optional project context. The `EntityRef` SHALL have a lossless canonical string form and SHALL round-trip parse from that form, so the same representation is shared by extraction, storage, and future resolution.

#### Scenario: Round-trip canonical form
- **WHEN** an `EntityRef { kind: LinearIssue, id: "AB-123", project: None }` is serialized to its canonical string and parsed back
- **THEN** the parsed reference equals the original

#### Scenario: Project context preserved
- **WHEN** an `EntityRef` for a merge request carries a project
- **THEN** the canonical string encodes the project and parsing restores it

### Requirement: Every relation carries evidence and provenance
The system SHALL represent each relation as a typed `Relation` with a subject `EntityRef`, a `RelationKind` predicate, an object `EntityRef`, a non-empty `evidence_record_id`, and a `Derivation` describing how it was produced. A relation without an `evidence_record_id` SHALL NOT be created.

#### Scenario: Relation names its evidence
- **WHEN** a relation is derived from a record
- **THEN** its `evidence_record_id` is the id of that record
- **AND** the id resolves to a record present in the source of truth or produced within the same batch

#### Scenario: Derivation distinguishes explicit from derived
- **WHEN** a relation comes from a record's own field (author, participant, thread)
- **THEN** its `Derivation` is `Explicit`
- **WHEN** a relation comes from a deterministic extraction rule
- **THEN** its `Derivation` is `Rule` and names the rule

### Requirement: Relations are explicit or deterministically derived
The system SHALL derive relations only from explicit record fields or pure, reproducible rules over those fields. The relation extractor SHALL be a pure function of its input records: given the same records it SHALL produce the same set of relations, in the same order.

#### Scenario: Determinism across runs
- **WHEN** the extractor runs twice over the same records
- **THEN** the two resulting relation sets are identical, including ordering

#### Scenario: Authorship edge from explicit field
- **WHEN** a record has a non-placeholder author
- **THEN** the extractor emits an `AuthoredBy` relation from the record entity to the author `Person`, with `Derivation::Explicit`

#### Scenario: Mention edge from extracted reference
- **WHEN** a record's entities contain a parsed reference such as a Linear issue
- **THEN** the extractor emits a `Mentions` or `References` relation to that entity, with a named `Rule` derivation

### Requirement: Semantic similarity never creates a relation
The system SHALL NOT create any relation from vector similarity, embedding distance, or other non-deterministic signals. Vectors serve retrieval ranking only and SHALL NOT be consulted by the relation extractor.

#### Scenario: Similar-but-unlinked records
- **WHEN** two records are semantically similar but neither states nor deterministically implies a link
- **THEN** the extractor produces no relation between them

### Requirement: Relation identity is stable and idempotent
The system SHALL assign each relation a stable key derived from its subject, predicate, object, evidence record id, and derivation, so that re-deriving the same relation is idempotent.

#### Scenario: Re-derivation does not duplicate
- **WHEN** the same relation is derived and stored twice
- **THEN** the graph contains exactly one relation for that key

