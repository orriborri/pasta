## ADDED Requirements

### Requirement: Git log parsing handles multi-paragraph commit messages
The system SHALL use null-byte/SOH record separators for git log parsing instead of splitting on double newlines.

#### Scenario: Commit with multi-paragraph body
- **WHEN** a git commit has a body containing blank lines (e.g., bullet lists separated by empty lines)
- **THEN** the entire body is captured in a single LogEntry, not split across multiple entries

#### Scenario: Commit with no body
- **WHEN** a git commit has only a subject line and no body
- **THEN** the entry is parsed correctly with an empty body field

### Requirement: Vault link repair uses document content, not frontmatter
The system SHALL strip YAML frontmatter before constructing the search query for finding related documents in `repair_links`.

#### Scenario: Document with standard frontmatter
- **WHEN** `repair_links` processes a vault document that starts with `---\nsource: vault\n...---`
- **THEN** the search query is built from the content after the frontmatter closing `---`

#### Scenario: Document with only frontmatter and short content
- **WHEN** the content after frontmatter is fewer than 50 characters
- **THEN** the document title (filename stem) is used as the search query instead
