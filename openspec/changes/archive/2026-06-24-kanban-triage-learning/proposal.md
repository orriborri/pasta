## Why

New tasks from feeds (GitLab, Slack, Gmail, Linear) currently get auto-tagged by the inbox-processor agent, giving the user no control over prioritization. The user wants to manage their own Kanban board — deciding what's "today" vs "later" — while pasta learns from those decisions to offer suggestions over time.

## What Changes

- **inbox-processor creates tasks without Kanban column tags** — new items land in "Uncategorized" on the Task List Kanban board
- **Triage pattern tracker** — a new module that observes when tasks get tagged (column assigned) and records source→column patterns
- **Daily note shows triage suggestions** — once patterns have enough confidence, the daily note lists untagged tasks with suggested columns based on history
- **Agent prompt updates** — inbox-processor stops auto-tagging, daily-writer includes triage section

## Capabilities

### New Capabilities
- `triage-learning`: Pattern-based column suggestions that learn from user's Kanban decisions

### Modified Capabilities
- `state-persistence`: Add triage pattern state tracking

## Impact

- `~/.kiro/pasta.toml` — agent prompt updates (no auto-tagging, daily note triage section)
- `crates/backend/src/fetchers/vault_manager.rs` — new function to scan for triage pattern changes
- `crates/common/src/vault.rs` — read triage patterns file
- New file: vault `0. Inbox/triage-patterns.json` — stores learned source→column mappings
