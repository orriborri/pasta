## Context

pasta is configured through multiple files in different locations and formats (TOML, JSON, hardcoded constants, env vars). The unified config consolidates everything into `~/.kiro/pasta.toml`.

## Goals / Non-Goals

**Goals:**
- Single file to understand and configure all pasta behavior
- Config loaded once at startup (no hot-reload)
- Auto-generate default config with helpful comments on first run
- All existing behavior preserved — just the config source changes

**Non-Goals:**
- Hot-reload of config (restart daemon to pick up changes)
- Config validation UI in TUI
- Migration tool for old config files (just regenerate defaults)

## Decisions

### 1. Config file structure

```toml
# ~/.kiro/pasta.toml

[general]
vault_path = "/home/orre/Obsidian/Readpeak"

[schedules.gitlab]
interval_minutes = 60
enabled = true

[schedules.linear]
interval_minutes = 60
enabled = true

[schedules.slack]
interval_minutes = 60
enabled = true

[schedules.gmail]
interval_minutes = 60
enabled = true

[schedules.calendar]
interval_minutes = 60
enabled = true

[schedules.vault-maintenance]
interval_minutes = 1440
enabled = true

[schedules.history-sync]
interval_minutes = 1440
enabled = true

[[agents]]
name = "inbox-processor"
prompt = "Process inbox and feeds into the vault."
cwd = "/home/orre/Obsidian/Readpeak"
depends_on = ["gitlab-fetcher", "gmail-fetcher", "slack-fetcher", "linear-fetcher"]

[[agents]]
name = "daily-writer"
prompt = "Create or update today's daily note."
cwd = "/home/orre/Obsidian/Readpeak"
depends_on = ["inbox-processor"]

[[repos]]
path = "/home/orre/ReadPeak/wiki"
include = ["*.md"]

[[repos]]
path = "/home/orre/ReadPeak/mononode"
include = ["README.md", "docs/**", "apps/platform/graphql/src/**/*.ts"]

[binaries]
glab = ""
slack_api = ""
linear_api = ""
kiro_cli = ""
```

**Rationale**: TOML maps naturally to the config structure. Sections are clear and self-documenting.

### 2. Global config singleton

Load config at daemon/CLI startup into a `once_cell::sync::Lazy<AppConfig>` static. All modules access it via `config::get()`.

**Rationale**: Config is read-only after startup. A global avoids threading config through every function. `once_cell` is already common in Rust ecosystems.

### 3. Vault path becomes dynamic

Replace `pub const VAULT_PATH: &str` with `pub fn vault_path() -> &'static str` that reads from config.

**Rationale**: Minimal change to call sites (add `()`) while removing the hardcoded path.

### 4. Drop TUI add/remove schedule

The `AddSchedule` and `RemoveSchedule` IPC commands are removed. Users edit `~/.kiro/pasta.toml` and restart the daemon. The TUI `a`/`d` keys on the schedule tab are removed.

**Rationale**: Agentic schedules change rarely. TOML editing is simpler than a TUI form, and avoids the complexity of writing TOML programmatically while preserving comments.

### 5. Binary path resolution

`[binaries]` section. Empty string = auto-detect from PATH. Non-empty = use that path.

**Rationale**: Same behavior as current env var fallback, but discoverable in config.

### 6. Agent last_run tracking

Agent `last_run` timestamps are no longer persisted in config (they were in `schedules.json`). They're tracked in-memory only, same as native schedules.

**Rationale**: last_run is runtime state, not config. Simplifies the file to be purely declarative.

## Risks / Trade-offs

- [Risk] Removing add/remove from TUI reduces convenience → Mitigation: schedules change rarely; a flash message "edit ~/.kiro/pasta.toml" guides user
- [Risk] Config parse failure on startup blocks the daemon → Mitigation: fall back to defaults, log error clearly
- [Trade-off] No migration from old files — user regenerates config. Acceptable since the app is single-user and iteration is fast.
