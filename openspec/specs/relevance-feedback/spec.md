# Relevance Feedback

## Purpose

Let users provide explicit good/bad judgments on search results and feed those judgments back into hybrid search ranking, so result quality improves over time per query.

## Requirements

### Requirement: User can mark search results as relevant or irrelevant
The system SHALL allow users to provide explicit feedback (good/bad) on individual search results, referencing them by record ID.

#### Scenario: Mark result as bad
- **WHEN** user runs `kb feedback <record-id> bad`
- **THEN** the judgment is stored with the most recent query context
- **AND** future searches for the same query penalize that record

#### Scenario: Mark result as good
- **WHEN** user runs `kb feedback <record-id> good`
- **THEN** the judgment is stored and future searches for the same query boost that record

### Requirement: Feedback adjusts future search ranking
The system SHALL apply score adjustments to hybrid search results based on accumulated feedback for matching queries.

#### Scenario: Previously penalized record ranks lower
- **WHEN** a record has 2 "bad" judgments for query "double verify"
- **AND** user searches "double verify" again
- **THEN** that record's RRF score is multiplied by 0.49 (0.7^2)

#### Scenario: Boost caps at maximum
- **WHEN** a record has 10 "good" judgments
- **THEN** the boost is capped at 3.0x (not 1.2^10)

### Requirement: Search output includes record IDs for feedback reference
The system SHALL display the record ID in each search result so users can reference it in feedback commands.

#### Scenario: Search shows IDs
- **WHEN** user runs `kb search "deployment"`
- **THEN** each result line includes the record ID (e.g., `[slack-C04AB-1234567.000]`)

### Requirement: Last query tracked for implicit feedback context
The system SHALL store the most recent search query so that `kb feedback` without `--query` uses it automatically.

#### Scenario: Feedback without explicit query
- **WHEN** user runs `kb search "auth migration"` followed by `kb feedback rec-123 bad`
- **THEN** the feedback is stored against query "auth migration"
