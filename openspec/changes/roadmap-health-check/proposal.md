## Why

There's no way to see at a glance whether roadmap initiatives are on track. The Gantt shows timing but doesn't calculate health. The user needs a daily summary comparing actual task progress against planned timelines to catch slippage early.

## What Changes

- **Daily note roadmap health table**: The daily-writer agent calculates initiative health (🟢/🟡/🔴) by comparing task completion % vs time elapsed % and writes a status table
- **Health calculation logic**: A Rust function that reads initiative target dates from `Roadmap/` and task completion from `Tasks/<initiative>/` to compute status

## Capabilities

### New Capabilities
- `roadmap-health`: Automated initiative health tracking comparing progress vs timeline

### Modified Capabilities
(none)

## Impact

- `crates/common/src/vault.rs` — add `compute_roadmap_health()` function
- `~/.pasta/config.toml` — daily-writer prompt updated to include health table
- Daily notes gain a `## 📊 Roadmap Health` section
