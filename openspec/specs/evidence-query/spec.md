# evidence-query Specification

## Purpose
TBD - created by archiving change structured-evidence-api. Update Purpose after archive.
## Requirements
### Requirement: Resolve an entity to a typed view
The system SHALL resolve a canonical `EntityRef` to a structured entity view reporting whether a backing record exists and the entity's in/out relation degree.

#### Scenario: Known entity with relations
- **WHEN** `get_entity` is called for an entity that has relations
- **THEN** it returns the entity, its in-degree and out-degree, and whether a backing record is present

#### Scenario: Entity referenced but never fetched
- **WHEN** `get_entity` is called for an entity that appears only as a relation endpoint (no record)
- **THEN** it returns the entity with `present = false` and the correct degree counts

### Requirement: List the relations incident to an entity
The system SHALL return the relations connected to an entity, optionally filtered by relation kind and bounded by a limit, each relation including its endpoints, `evidence_record_id`, and `Derivation`, ordered deterministically.

#### Scenario: Both directions returned
- **WHEN** `get_related` is called for an entity that is the subject of some relations and the object of others
- **THEN** both incoming and outgoing relations are returned

#### Scenario: Filter by relation kind
- **WHEN** `get_related` is called with a kind filter of `References`
- **THEN** only `References` relations are returned

#### Scenario: Deterministic ordering and limit
- **WHEN** `get_related` is called twice with the same limit
- **THEN** the two results are identical in content and order

### Requirement: Resolve evidence to the underlying record
The system SHALL resolve an `evidence_record_id` to the underlying record, returning a structured evidence view (source, kind, title, snippet, url, timestamp). Records are read from Parquet, the source of truth.

#### Scenario: Evidence record present
- **WHEN** `get_evidence` is called with a record id that exists in Parquet
- **THEN** it returns the structured evidence view for that record

#### Scenario: Evidence record absent
- **WHEN** `get_evidence` is called with a record id that has no record in Parquet
- **THEN** it returns an empty result rather than fabricating content

### Requirement: Assemble an entity context bundle
The system SHALL assemble, in one call, an entity's view, its top relations, and the distinct evidence records behind those relations, introducing no facts beyond what the graph and Parquet already hold.

#### Scenario: Context bundles entity, relations, and evidence
- **WHEN** `get_context` is called for an entity with relations
- **THEN** it returns the entity view, the relations (each with evidence pointer and derivation), and the evidence records for those relations

#### Scenario: Context contains only grounded data
- **WHEN** `get_context` is assembled
- **THEN** every relation traces to an `evidence_record_id` and every evidence record corresponds to a stored record

### Requirement: Produce a time-ordered timeline for an entity
The system SHALL produce a time-ordered list of the evidence records touching an entity, ordered by the record's timestamp with a stable tie-break, for "what happened with this entity" views.

#### Scenario: Timeline ordered by event time
- **WHEN** `get_timeline` is called for an entity connected to records at different times
- **THEN** the entries are ordered by the evidence record's `created_at`

#### Scenario: Timeline is deterministic
- **WHEN** `get_timeline` is called twice for the same entity
- **THEN** the two timelines are identical in content and order

### Requirement: Structured search results carry typed fields
The system SHALL return search results as typed hits carrying `record_id`, source, title, snippet, url, timestamp, and score, rather than a single prose blob, reusing the `record_id` surfaced by the relevance-feedback work.

#### Scenario: Search returns typed hits
- **WHEN** the structured search is invoked with a query
- **THEN** each hit is a typed record with a `record_id` field usable for follow-up queries and feedback

#### Scenario: Source filter applied
- **WHEN** the structured search is invoked with a source filter
- **THEN** only hits from that source are returned

