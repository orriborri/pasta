# Ingestion Pipeline

## Purpose

Multi-stage pre-processing pipeline that transforms raw fetched data through normalize, deduplicate, filter, extract, summarize, and enrich stages before storage.

## Requirements

### Requirement: Multi-stage pre-processing pipeline
The system SHALL process raw fetched data through ordered stages: normalize → deduplicate → filter → extract entities → summarize → enrich, before chunking and embedding.

#### Scenario: Slack message thread processed
- **WHEN** a 50-message Slack thread is ingested
- **THEN** it passes through normalize (resolve user IDs to names), filter (skip bot messages), summarize (compress to key decisions), then chunk the summary for embedding

#### Scenario: Stage is disabled
- **WHEN** the summarize stage is disabled in config
- **THEN** the pipeline skips it and passes records directly from extract to chunk

### Requirement: Normalize stage resolves identities
The system SHALL resolve author identifiers (Slack user IDs, GitLab usernames, email addresses) to canonical person names using the People/ directory as lookup.

#### Scenario: Slack user ID resolved
- **WHEN** a message has author "U04ABCDEF"
- **AND** People/Tuomo Tilli.md has `slack_id: U04ABCDEF`
- **THEN** the normalized author field becomes "Tuomo Tilli"

### Requirement: Exact deduplication runs early
The system SHALL detect and skip byte-identical duplicate content using content hashing, early in the pipeline (before entity extraction).

#### Scenario: Identical record re-fetched
- **WHEN** the same record is fetched twice with identical content
- **THEN** only one copy proceeds through the pipeline

### Requirement: Cross-source deduplication runs after entity extraction
The system SHALL merge records that reference the same work item across sources, AFTER the extract stage has identified the shared entity.

#### Scenario: Same notification in Slack and Gmail
- **WHEN** a GitLab MR notification appears in both Slack and Gmail
- **AND** the extract stage has tagged both with `mr:!2935`
- **THEN** they are merged into one record with source metadata noting both origins

### Requirement: Filter stage removes noise
The system SHALL skip bot messages, CI notifications, auto-generated content, and promotional emails unless explicitly configured to include them.

#### Scenario: CI bot message
- **WHEN** a Slack message is from "GitLab Bot" containing pipeline status
- **THEN** it is filtered out and not indexed

### Requirement: Entity extraction identifies structured references
The system SHALL extract mentions of people, projects, MRs/PRs, Linear issues, and URLs from content and store them as structured entity fields.

#### Scenario: MR mentioned in Slack
- **WHEN** a message contains "reviewed !2918"
- **THEN** entities field includes `mr:!2918` linked to the GitLab MR

### Requirement: Summarize stage compresses long content
The system SHALL summarize threads longer than a configurable threshold (default: 10 messages) into a concise summary + key decisions, optionally using an LLM.

#### Scenario: Long thread summarized
- **WHEN** a Slack thread has 30 messages discussing a deployment issue
- **THEN** it is stored as a ~200 word summary plus extracted decisions/action items

#### Scenario: LLM unavailable
- **WHEN** the LLM endpoint is unreachable
- **THEN** fallback to heuristic: first message + last message + messages with reactions

### Requirement: Pipeline stages are composable traits
Each stage SHALL implement a `PipelineStage` trait accepting and returning `Vec<Record>` (plain structs, not Arrow batches), allowing reordering and conditional execution via configuration.

#### Scenario: Custom pipeline order
- **WHEN** config specifies `stages = ["normalize", "filter", "extract"]` (skipping dedupe and summarize)
- **THEN** only those three stages run in that order
