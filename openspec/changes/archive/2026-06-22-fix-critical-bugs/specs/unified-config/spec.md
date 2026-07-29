## MODIFIED Requirements

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
