## ADDED Requirements

### Requirement: Initiative health computed from progress vs timeline
The system SHALL calculate health status for each roadmap initiative by comparing task completion percentage against time elapsed percentage.

#### Scenario: Initiative on track
- **WHEN** Monitoring has 2/3 tasks done and 60% of the timeline has elapsed
- **THEN** health is 🟢 On track (67% done ≥ 60% elapsed)

#### Scenario: Initiative at risk
- **WHEN** Pipelines has 0/3 tasks done and 30% of the timeline has elapsed
- **THEN** health is 🟡 At risk (0% done, within 20% tolerance of 30%)

#### Scenario: Initiative off track
- **WHEN** Data Platform has 0/2 tasks done and 60% of the timeline has elapsed
- **THEN** health is 🔴 Off track (0% done, significantly behind 60%)

#### Scenario: Initiative past due and incomplete
- **WHEN** today is past the initiative's end date and tasks remain incomplete
- **THEN** health is 🔴 Off track

#### Scenario: Initiative not yet started
- **WHEN** today is before the initiative's start date
- **THEN** health is 🟢 On track

### Requirement: Health table in daily note
The daily-writer agent SHALL include a `## 📊 Roadmap Health` section with a table showing each initiative's progress, target date, and health status.

#### Scenario: Daily note generated with health section
- **WHEN** the daily-writer runs
- **THEN** the daily note contains a table with columns: Initiative, Progress, Target, Status

#### Scenario: Initiative has no dates
- **WHEN** a Roadmap file has no `start:` or `end:` frontmatter
- **THEN** it is omitted from the health table

### Requirement: Health data available via MCP
The system SHALL expose roadmap health through the MCP server so AI tools can query initiative status.

#### Scenario: AI assistant asks about roadmap status
- **WHEN** an MCP tool call requests roadmap health
- **THEN** it returns the same data as the daily note health table
