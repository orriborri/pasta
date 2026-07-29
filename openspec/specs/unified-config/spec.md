# Unified Config

## Purpose

Centralize all pasta configuration in a single `~/.pasta/config.toml` file loaded at startup, with sensible defaults and no runtime mutation.

## Requirements

### Requirement: Single config file at ~/.pasta/config.toml
The system SHALL load all configuration from `~/.pasta/config.toml` at startup. The `[general]` section SHALL include `agent_timeout_minutes` (default 10).

#### Scenario: Config file exists and is valid
- **WHEN** daemon or CLI starts and `~/.pasta/config.toml` exists with valid TOML
- **THEN** all settings are loaded from that file

#### Scenario: Config file missing
- **WHEN** daemon or CLI starts and `~/.pasta/config.toml` does not exist
- **THEN** a default config file is generated with comments and the app starts with defaults

#### Scenario: Config file has parse errors
- **WHEN** `~/.pasta/config.toml` contains invalid TOML
- **THEN** system logs an error, falls back to defaults, and continues startup

#### Scenario: Custom agent timeout
- **WHEN** config has `[general] agent_timeout_minutes = 5`
- **THEN** agents are killed after 5 minutes of runtime

### Requirement: Vault path from config
The vault path SHALL be read from `[general] vault_path` in the config file.

#### Scenario: Custom vault path
- **WHEN** config contains `vault_path = "/some/path"`
- **THEN** all vault operations use that path

### Requirement: Native schedules from config
Native schedule intervals and enabled state SHALL be read from `[schedules.<name>]` sections.

#### Scenario: Custom interval
- **WHEN** config has `[schedules.gmail]` with `interval_minutes = 30`
- **THEN** gmail fetcher runs every 30 minutes

#### Scenario: Schedule disabled
- **WHEN** config has `[schedules.calendar]` with `enabled = false`
- **THEN** calendar fetcher does not run

### Requirement: Agentic schedules from config
Agentic schedules SHALL be defined as `[[agents]]` entries in the config file.

#### Scenario: Agent with dependencies
- **WHEN** config has an `[[agents]]` entry with `depends_on = ["gitlab-fetcher"]`
- **THEN** that agent triggers when its dependencies complete

#### Scenario: Agent with interval
- **WHEN** config has an `[[agents]]` entry with `interval_minutes = 120`
- **THEN** that agent runs every 120 minutes

### Requirement: Repo index from config
Repository indexing configuration SHALL be defined as `[[repos]]` entries in the config file.

#### Scenario: Repo with include patterns
- **WHEN** config has `[[repos]]` with `path` and `include` patterns
- **THEN** history sync indexes matching files from that repo

### Requirement: Binary paths from config
Binary paths SHALL be read from `[binaries]` section. Empty string means auto-detect from PATH. All modules that invoke external binaries SHALL use the `resolve_binary` function, including `history/sync_slack.rs` and `acp.rs`.

#### Scenario: Custom binary path
- **WHEN** config has `[binaries] glab = "/usr/local/bin/glab"`
- **THEN** that path is used for glab commands

#### Scenario: Empty binary path (auto-detect)
- **WHEN** config has `[binaries] glab = ""`
- **THEN** glab is resolved from PATH

#### Scenario: sync_slack uses configured slack-api path
- **WHEN** `history/sync_slack.rs` runs and config has `[binaries] slack_api = "/opt/bin/slack-api"`
- **THEN** that path is used instead of the hardcoded default

#### Scenario: acp uses configured kiro-cli path
- **WHEN** `acp.rs` spawns a chat session and config has `[binaries] kiro_cli = "/usr/bin/kiro-cli"`
- **THEN** that path is used instead of the hardcoded `/home/orre/.nix-profile/bin/kiro-cli`

### Requirement: No runtime config mutation
The config file SHALL NOT be modified at runtime. Schedule add/remove from TUI is removed.

#### Scenario: User wants to add an agent
- **WHEN** user tries to add a schedule
- **THEN** TUI shows flash "Edit ~/.pasta/config.toml to add agents"
