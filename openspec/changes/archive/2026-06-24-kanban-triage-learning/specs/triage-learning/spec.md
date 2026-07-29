## ADDED Requirements

### Requirement: New tasks arrive without column tags
The inbox-processor agent SHALL create task files without Kanban column tags (#today, #this-week, #later, #backlog). Tasks appear in the "Uncategorized" column for manual triage.

#### Scenario: GitLab MR review arrives
- **WHEN** the inbox-processor creates a task from a GitLab MR review notification
- **THEN** the task file has no column tag in its `tags:` frontmatter field

#### Scenario: Urgent Linear issue arrives
- **WHEN** the inbox-processor creates a task from a P1 Linear issue
- **THEN** the task file has no column tag — urgency is for the user to decide

### Requirement: Pattern tracker observes column assignments
The system SHALL scan task files during vault-maintenance to detect when a column tag has been added (by the user via Kanban drag) and record the source→column mapping.

#### Scenario: User drags a GitLab MR review task to "today"
- **WHEN** vault-maintenance finds a task with `source: gitlab`, title containing "review", and `tags: [today]`
- **THEN** the pattern `{source: "gitlab", keywords: ["review"], column: "today"}` has its hits incremented

#### Scenario: User places a budget alert in "this-week"
- **WHEN** vault-maintenance finds a task with `source: gmail`, title containing "budget", and `tags: [this-week]`
- **THEN** the pattern `{source: "gmail", keywords: ["budget"], column: "this-week"}` has its hits incremented

#### Scenario: User contradicts an existing pattern
- **WHEN** a task matches pattern keywords but is placed in a different column than the pattern predicts
- **THEN** the pattern's misses count is incremented

### Requirement: Suggestions appear in daily note after threshold
The daily-writer agent SHALL include a triage section showing untagged tasks with column suggestions when patterns have sufficient confidence (≥3 hits, <30% miss rate).

#### Scenario: Confident pattern exists for untagged task
- **WHEN** an untagged task matches a pattern with hits=5, misses=0
- **THEN** the daily note shows `- [[Task]] → 💡 #today (pattern: MR review, 5/5)`

#### Scenario: No pattern matches
- **WHEN** an untagged task doesn't match any pattern
- **THEN** the daily note shows `- [[Task]] — no suggestion`

#### Scenario: Pattern below confidence threshold
- **WHEN** a task matches a pattern with hits=2 (below threshold of 3)
- **THEN** no suggestion is shown for that task

### Requirement: Patterns decay over time
The system SHALL reduce pattern hit counts by 1 per month and remove patterns with 0 hits, preventing stale patterns from persisting indefinitely.

#### Scenario: Monthly decay
- **WHEN** vault-maintenance runs and a pattern's last update was >30 days ago
- **THEN** hits is decremented by 1

#### Scenario: Pattern reaches zero
- **WHEN** a pattern's hits reach 0 after decay
- **THEN** the pattern is removed from the patterns file
