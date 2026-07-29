## ADDED Requirements

### Requirement: Initiative embeddings available for routing queries
The system SHALL ensure initiative description content is indexed in LanceDB so that vector similarity search can match tasks against initiatives.

#### Scenario: New initiative added to Roadmap
- **WHEN** a new file `Roadmap/ML Platform.md` is created
- **AND** the next history-sync runs
- **THEN** its content is embedded and searchable in LanceDB for routing comparisons
