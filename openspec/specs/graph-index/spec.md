# graph-index Specification

## Purpose
TBD - created by archiving change typed-evidence-graph. Update Purpose after archive.
## Requirements
### Requirement: Graph is a rebuildable derived index
The system SHALL store the evidence graph in a SQLite database (`graph.db`) treated as a derived index, sibling to Tantivy and LanceDB. The graph SHALL be fully rebuildable from Parquet and SHALL NOT be a source of truth.

#### Scenario: Graph deleted and rebuilt
- **WHEN** `graph.db` is deleted
- **AND** `kb reindex` runs
- **THEN** the graph is rebuilt from Parquet without re-fetching from any source API
- **AND** the rebuilt graph contains the same entities and relations as before

#### Scenario: Rebuild is deterministic
- **WHEN** the graph is rebuilt twice from the same Parquet data
- **THEN** the two rebuilds contain identical entities and relations

### Requirement: Graph store persists entities and relations with evidence
The system SHALL persist entities keyed by canonical reference and relations keyed by stable relation key, storing each relation's predicate, endpoints, `evidence_record_id`, derivation, and timestamp. Relation endpoints SHALL reference stored entities.

#### Scenario: Endpoint entity without a backing record
- **WHEN** a relation references an entity that has no record yet (e.g. a mentioned ticket not fetched)
- **THEN** the referenced entity is still stored as a graph entity
- **AND** the relation's `evidence_record_id` points at the record that mentioned it

#### Scenario: Evidence lookup
- **WHEN** relations are stored
- **THEN** they can be looked up by `evidence_record_id`, by subject, and by object

### Requirement: Incremental sync keeps the graph current
The system SHALL upsert relations derived from a sync batch into the graph after the batch is written to Parquet, using the stable relation key so repeated syncs do not duplicate relations.

#### Scenario: New batch adds relations
- **WHEN** a sync batch produces new records and relations
- **THEN** the new relations are upserted into `graph.db`
- **AND** re-running the same batch does not create duplicate relations

#### Scenario: Derived write fails during sync
- **WHEN** records are written to Parquet but the graph upsert fails
- **THEN** the records are preserved in Parquet, the error is logged, and the relations are regenerated on the next `kb reindex`

