## MODIFIED Requirements

### Requirement: Single config file at ~/.kiro/pasta.toml
The system SHALL load all configuration from `~/.kiro/pasta.toml` at startup. The `[general]` section SHALL include `agent_timeout_minutes` (default 10).

#### Scenario: Config file exists and is valid
- **WHEN** daemon or CLI starts and `~/.kiro/pasta.toml` exists with valid TOML
- **THEN** all settings are loaded from that file

#### Scenario: Config file missing
- **WHEN** daemon or CLI starts and `~/.kiro/pasta.toml` does not exist
- **THEN** a default config file is generated with comments and the app starts with defaults

#### Scenario: Config file has parse errors
- **WHEN** `~/.kiro/pasta.toml` contains invalid TOML
- **THEN** system logs an error, falls back to defaults, and continues startup

#### Scenario: Custom agent timeout
- **WHEN** config has `[general] agent_timeout_minutes = 5`
- **THEN** agents are killed after 5 minutes of runtime
