## ADDED Requirements

### Requirement: Full-text search via Tantivy
The system SHALL maintain a Tantivy inverted index for keyword search, updated incrementally alongside vector embeddings.

#### Scenario: Exact keyword search
- **WHEN** user searches for "EventBridge"
- **THEN** Tantivy returns all records containing that exact term, ranked by TF-IDF

### Requirement: Hybrid search merges vector and full-text results
The system SHALL run both vector (LanceDB) and full-text (Tantivy) searches in parallel, merging results using Reciprocal Rank Fusion.

#### Scenario: Semantic + keyword combined
- **WHEN** user searches "how do we handle auth token refresh"
- **THEN** vector search finds semantically similar discussions AND full-text finds documents mentioning "auth token refresh" literally, merged into one ranked list

#### Scenario: Full-text only when semantic fails
- **WHEN** a search term is a unique identifier like "MR-2918" with no semantic meaning
- **THEN** full-text search finds exact matches that vector search would miss
