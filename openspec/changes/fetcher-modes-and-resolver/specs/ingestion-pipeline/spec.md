## MODIFIED Requirements

### Requirement: Multi-stage pre-processing pipeline
The system SHALL process raw fetched data in three phases: pre-stages (normalize → deduplicate → filter → extract entities), an asynchronous resolve phase that closes entity references via on-demand fetching, and post-stages (cross-source dedup → summarize → enrich), before chunking and embedding. Pre-stages and post-stages are synchronous `PipelineStage` implementations; the resolve phase is an async orchestrator between them.

#### Scenario: Slack message thread processed
- **WHEN** a 50-message Slack thread is ingested
- **THEN** it passes through pre-stages (normalize, filter, extract), the resolve phase, then post-stages (summarize compresses to key decisions), then chunk the summary for embedding

#### Scenario: Stage is disabled
- **WHEN** the summarize stage is disabled in config
- **THEN** the pipeline skips it and passes records directly from the resolve phase to chunk

#### Scenario: Resolve phase pulls a referenced entity
- **WHEN** pre-stages extract a reference to an entity absent from the knowledge base
- **THEN** the resolve phase fetches it before post-stages run, so cross-source dedup can merge it

### Requirement: Cross-source deduplication runs after entity extraction
The system SHALL merge records that reference the same work item across sources, AFTER the extract stage has identified the shared entity AND after the resolve phase has fetched any referenced entities missing from the knowledge base.

#### Scenario: Same notification in Slack and Gmail
- **WHEN** a GitLab MR notification appears in both Slack and Gmail
- **AND** the extract stage has tagged both with `mr:!2935`
- **THEN** they are merged into one record with source metadata noting both origins

#### Scenario: Commit merges with a just-resolved ticket
- **WHEN** a commit tagged `linear:AB-123` is ingested and AB-123 was fetched during the resolve phase
- **THEN** cross-source dedup merges the commit and the ticket record into one linked record
