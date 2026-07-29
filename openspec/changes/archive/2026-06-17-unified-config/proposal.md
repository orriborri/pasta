## Why

Configuration is scattered across 3 files (`~/.kiro/native-schedules.toml`, `~/.kiro/schedules.json`, `.history/repo-index.toml`), one hardcoded constant (`VAULT_PATH`), and environment variables. This makes it hard to understand what pasta is configured to do, and requires editing multiple files in different locations. A single `~/.kiro/pasta.toml` provides one place to configure everything.

## What Changes

- Introduce `~/.kiro/pasta.toml` as the single source of truth for all app configuration
- Remove `~/.kiro/native-schedules.toml`, `~/.kiro/schedules.json`, and `.history/repo-index.toml`
- Make `VAULT_PATH` configurable instead of hardcoded
- Move agentic schedules from JSON to TOML (drop TUI add/remove — edit the file instead)
- Binary paths configurable in TOML instead of env vars
- Generate default config on first run with comments explaining each section

## Capabilities

### New Capabilities
- `unified-config`: Single TOML configuration file at `~/.kiro/pasta.toml` covering vault path, native schedules, agentic schedules, repo indexing, and binary paths

### Modified Capabilities

## Impact

- `crates/common/src/data.rs` — replace `load_native_config()`, `load_schedules()`, `save_schedules()` with unified config loader
- `crates/common/src/vault.rs` — `VAULT_PATH` becomes dynamic from config
- `crates/backend/src/history/config.rs` — repo config loaded from unified config
- `crates/backend/src/scheduler.rs` — read agentic schedules from config
- `crates/backend/src/fetchers/mod.rs` — binary paths from config
- `crates/tui/src/main.rs` — remove add/remove schedule commands (edit TOML instead)
- `crates/common/src/ipc.rs` — remove `AddSchedule`/`RemoveSchedule` commands
- Delete: `~/.kiro/native-schedules.toml`, `~/.kiro/schedules.json`, `.history/repo-index.toml`
