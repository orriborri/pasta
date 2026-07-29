## ADDED Requirements

### Requirement: Single config file at ~/.kiro/pasta.toml
The system SHALL load all configuration from `~/.kiro/pasta.toml` at startup.

#### Scenario: Config file exists and is valid
- **WHEN** daemon or CLI starts and `~/.kiro/pasta.toml` exists with valid TOML
- **THEN** all settings are loaded from that file

#### Scenario: Config file missing
- **WHEN** daemon or CLI starts and `~/.kiro/pasta.toml` does not exist
- **THEN** a default config file is generated with comments and the app starts with defaults

#### Scenario: Config file has parse errors
- **WHEN** `~/.kiro/pasta.toml` contains invalid TOML
- **THEN** system logs an error, falls back to defaults, and continues startup

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
Binary paths SHALL be read from `[binaries]` section. Empty string means auto-detect from PATH.

#### Scenario: Custom binary path
- **WHEN** config has `[binaries] glab = "/usr/local/bin/glab"`
- **THEN** that path is used for glab commands

#### Scenario: Empty binary path (auto-detect)
- **WHEN** config has `[binaries] glab = ""`
- **THEN** glab is resolved from PATH

### Requirement: No runtime config mutation
The config file SHALL NOT be modified at runtime. Schedule add/remove from TUI is removed.

#### Scenario: User wants to add an agent
- **WHEN** user tries to add a schedule
- **THEN** TUI shows flash "Edit ~/.kiro/pasta.toml to add agents"
