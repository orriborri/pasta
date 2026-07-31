## ADDED Requirements

### Requirement: Vault paths resolve through a single layout type
The system SHALL expose every vault-relative path (task directory, inbox, daily notes, archive, PARA folders, people, roadmap, feeds, triage patterns) as a method on one `VaultLayout` type in `pasta_common::vault`. No other module SHALL construct these paths from string literals.

#### Scenario: Task directory referenced from multiple crates
- **WHEN** the backend, TUI, and Trello sync each need the task directory
- **THEN** all three call the same layout method, and no crate contains the literal `"Tasks"`

#### Scenario: Daily note path referenced from multiple modules
- **WHEN** the daily-note generator, TUI, and vault helpers each need today's note
- **THEN** all of them call one layout method rather than each formatting `0. Inbox/Daily/<date>.md`

### Requirement: Feeds directory is vault-relative everywhere
The system SHALL resolve the feeds directory relative to the configured vault path in every crate, including `kb-sync`.

#### Scenario: Vault path differs from the developer's home directory
- **WHEN** `[general] vault_path` points somewhere other than `/home/orre/Obsidian/Readpeak`
- **THEN** feed markdown is written under that vault, not to a hardcoded absolute path

### Requirement: Missing vault folders are reported, not silently skipped
The system SHALL log a warning naming the missing directory when a layout path does not exist, instead of treating an unreadable directory as empty.

#### Scenario: PARA folder renamed by the user
- **WHEN** a user renames `1. Projects` and the PARA audit runs
- **THEN** a warning names the missing directory rather than the audit reporting zero documents

#### Scenario: Archive directory absent
- **WHEN** task archiving runs and the archive directory does not exist
- **THEN** the directory is created at the layout-defined location and the action is logged
