## ADDED Requirements

### Requirement: Agents tab lists available agents
The TUI SHALL display a tab listing agents discovered from the filesystem.

#### Scenario: Viewing the Agents tab
- **WHEN** user presses 6 or navigates to the Agents tab
- **THEN** a list of agents is shown with their name and description

### Requirement: Launch agent from tab
The TUI SHALL allow launching an interactive chat session with the selected agent by pressing Enter.

#### Scenario: User selects an agent and presses Enter
- **WHEN** user selects "good-morning" and presses Enter
- **THEN** chat mode opens with the good-morning agent connected

### Requirement: Chat header shows active agent name
The TUI SHALL display the name of the active agent in the chat mode header.

#### Scenario: Chat started with end-of-day agent
- **WHEN** chat mode is active with the end-of-day agent
- **THEN** the header shows "Chat: end-of-day"

### Requirement: Agent discovery from filesystem
The TUI SHALL discover agents from `~/.kiro/agents/` and the vault's `.kiro/agents/` directory.

#### Scenario: Agents from both directories
- **WHEN** agents exist in both global and vault-local directories
- **THEN** all agents are shown, with vault-local taking precedence on name conflicts
