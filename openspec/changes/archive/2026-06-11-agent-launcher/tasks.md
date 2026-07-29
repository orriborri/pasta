## 1. Agent discovery

- [x] 1.1 Add agent discovery function that scans `~/.kiro/agents/` and vault `.kiro/agents/` for JSON files, parsing name and description

## 2. TUI changes

- [x] 2.1 Add `Agents` variant to `Tab` enum, update tab navigation and keybinding (`6`)
- [x] 2.2 Store discovered agents and active chat agent name in `App`
- [x] 2.3 Render Agents tab with selectable list (name + description)
- [x] 2.4 Handle Enter on Agents tab: enter chat mode with selected agent name
- [x] 2.5 Update chat header to show active agent name instead of hardcoded "Weekly Review Chat"

## 3. Verify

- [x] 3.1 Build and verify compilation
