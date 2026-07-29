## 1. Agent Prompt Updates

- [x] 1.1 Update inbox-processor prompt in `~/.kiro/pasta.toml` to stop adding column tags — tasks arrive untagged
- [x] 1.2 Update daily-writer prompt to include a triage suggestions section for untagged tasks

## 2. Pattern Tracker

- [x] 2.1 Add `triage_patterns_path()` helper to `crates/common/src/vault.rs` returning `<vault>/0. Inbox/triage-patterns.json`
- [x] 2.2 Add `TriagePattern` struct and `load_patterns`/`save_patterns` functions to `crates/common/src/vault.rs`
- [x] 2.3 In `crates/backend/src/fetchers/vault_manager.rs`, add `update_triage_patterns()` that scans `Tasks/` for files with both a `source:` field and a column tag, then updates pattern hits/misses
- [x] 2.4 Call `update_triage_patterns()` from `vault_manager::run()`

## 3. Pattern Decay

- [x] 3.1 In `update_triage_patterns()`, check `last_updated` on each pattern — if >30 days, decrement hits by 1 and remove if zero

## 4. Daily Note Triage Section

- [x] 4.1 Add `suggest_columns()` to `crates/common/src/vault.rs` that takes untagged tasks + patterns and returns suggestions (column + confidence)
- [x] 4.2 The daily-writer agent prompt includes instructions to call this logic and format the triage section

## 5. Initial Patterns Seeding

- [x] 5.1 Create initial `0. Inbox/triage-patterns.json` seeded from existing tagged tasks in `Tasks/` (bootstrap from current state)
