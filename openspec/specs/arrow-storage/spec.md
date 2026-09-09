# Arrow Storage

## Purpose

Columnar storage layer for kb-engine using Apache Parquet as the authoritative data store, with derived indexes (LanceDB, Tantivy) rebuildable from it.
## Requirements
### Requirement: Raw data stored as Parquet
The system SHALL store all ingested records as Apache Parquet files, partitioned by source and month.

#### Scenario: Gmail ingested
- **WHEN** 200 Gmail threads from June 2026 are processed
- **THEN** they are written to `data/raw/gmail/2026-06.parquet`

#### Scenario: Re-process from raw
- **WHEN** the pipeline config changes (e.g., new entity extraction rules)
- **THEN** the system can replay the pipeline from Parquet without re-fetching from APIs

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

### Requirement: Records use stable IDs with replace-by-ID updates
Each record SHALL have a deterministic ID derived from source and native identifier. Re-syncing a changed item replaces the existing record by ID across all stores.

#### Scenario: Slack thread gets a new reply
- **WHEN** a previously-indexed thread receives a new message and is re-synced
- **THEN** the existing record (same ID) is replaced in Parquet, LanceDB, and Tantivy — not duplicated

#### Scenario: Unchanged record skipped
- **WHEN** a re-synced record's content hash matches the stored hash
- **THEN** it is skipped — no re-embedding, no re-write

### Requirement: Universal schema for all sources
All records SHALL conform to a universal schema (id, source, kind, title, content, summary, author, participants, created_at, updated_at, url, thread_id, entities, tags, project). The pipeline processes these as plain Rust `Record` structs; conversion to Arrow occurs only at the ParquetStore write boundary.

#### Scenario: Pipeline processes row-wise
- **WHEN** a record passes through extract/summarize/enrich stages
- **THEN** it is a plain `Record` struct (no Arrow conversion), enabling natural regex and LLM operations

#### Scenario: Slack and Gmail in same query
- **WHEN** a DataFusion SQL query filters by `created_at > '2026-06-01'`
- **THEN** results include both Slack messages and Gmail threads in the same result set

### Requirement: DataFusion SQL queries over stored data
The system SHALL support SQL queries over Parquet files using DataFusion for structured filtering and aggregation.

#### Scenario: Count messages per person per week
- **WHEN** user queries `SELECT author, COUNT(*) FROM records WHERE source='slack' GROUP BY author`
- **THEN** DataFusion executes over Parquet and returns results

