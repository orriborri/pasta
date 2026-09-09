## MODIFIED Requirements

### Requirement: Parquet is source of truth, derived indexes rebuildable
Parquet SHALL be authoritative. LanceDB (vectors), Tantivy (full-text), and the evidence graph (`graph.db`) are derived indexes fully rebuildable from Parquet.

#### Scenario: Derived index corruption
- **WHEN** the LanceDB, Tantivy, or graph index is corrupted or deleted
- **THEN** `kb reindex` rebuilds it from Parquet without re-fetching from any source API

#### Scenario: Derived write fails during sync
- **WHEN** a record is written to Parquet but the LanceDB upsert fails
- **THEN** the record is preserved in Parquet, the error is logged, and it is re-indexed on the next reindex (no data loss)

#### Scenario: Graph rebuilt during reindex
- **WHEN** `kb reindex` runs
- **THEN** it regenerates `graph.db` from the records read out of Parquet, deterministically, alongside the vector and text indexes
