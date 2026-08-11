## MODIFIED Requirements

### Requirement: Pipeline stages are composable traits
Each stage SHALL implement a `PipelineStage` trait accepting and returning `Vec<Record>` (plain structs, not Arrow batches), allowing reordering and conditional execution via configuration. The pipeline SHALL be callable as a library function from both the `kb` CLI and the backend fetch cycle via the `kb-sync` crate.

#### Scenario: Custom pipeline order
- **WHEN** config specifies `stages = ["normalize", "filter", "extract"]` (skipping dedupe and summarize)
- **THEN** only those three stages run in that order

#### Scenario: Pipeline called from backend fetch cycle
- **WHEN** the backend fetch cycle completes fetching
- **THEN** it calls `kb_sync::run()` which invokes the pipeline inline
- **AND** records are stored to Parquet, Tantivy, and LanceDB

#### Scenario: Pipeline called from kb CLI
- **WHEN** user runs `kb sync`
- **THEN** it calls the same `kb_sync::run()` function
- **AND** behavior is identical to the backend invocation
