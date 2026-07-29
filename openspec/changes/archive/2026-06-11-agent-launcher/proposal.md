## Why

The `good-morning` and `end-of-day` agents are the primary daily workflow agents. Currently there's no way to launch them from inside pasta — the chat mode hardcodes `weekly-reviewer`. Users need a visible list of available agents they can pick from and launch into a chat session.

## What Changes

- Add a new "Agents" tab to the TUI listing available agents (good-morning, end-of-day, weekly-reviewer)
- Pressing Enter on an agent launches chat mode with that agent
- Discover agents from `~/.kiro/agents/` and the vault's `.kiro/agents/` directory

## Capabilities

### New Capabilities
- `agents-tab`: A new TUI tab showing available agents that can be launched into interactive chat sessions

### Modified Capabilities

## Impact

- `crates/tui/src/main.rs` — add Agents tab handling, Enter launches chat with selected agent
- `crates/tui/src/ui.rs` — new tab enum variant, agent list rendering, chat header shows agent name
- `crates/common/src/ipc.rs` — no protocol change (ChatStart already accepts an agent name)
