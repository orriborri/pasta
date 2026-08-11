## ADDED Requirements

### Requirement: Vault organization is independent of knowledge-base indexing
Pasta SHALL provide daily-note generation and vault upkeep (index, link repair, PARA audit) from a module that does not depend on the knowledge-base ingestion/index code. Removing kb-engine indexing SHALL NOT remove these features.

#### Scenario: Daily note generated without history index
- **WHEN** the daily-writer runs after the `history/` ingestion modules are removed
- **THEN** the daily note is generated successfully using the relocated vault-organization module

#### Scenario: Vault organize runs standalone
- **WHEN** the user triggers vault organize (index, repair links, PARA audit)
- **THEN** it completes without invoking any knowledge-base indexer

### Requirement: Related-document lookups use kb-engine search
Where vault organization needs related documents (e.g. link repair), it SHALL query kb-engine hybrid search rather than a pasta-internal index.

#### Scenario: Link repair finds related notes
- **WHEN** link repair searches for documents related to a note
- **THEN** the candidates come from kb-engine hybrid search results
