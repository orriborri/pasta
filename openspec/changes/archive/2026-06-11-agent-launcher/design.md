## Context

The TUI has tabs: Tasks [1], Schedule [2], People [3], Feeds [4], Stats [5]. Chat mode is a separate fullscreen overlay triggered by a key. Agents live as JSON files in `~/.kiro/agents/` (global) and `/home/orre/Obsidian/Readpeak/.kiro/agents/` (vault-local).

## Goals / Non-Goals

**Goals:**
- New "Agents [6]" tab showing a selectable list of available agents
- Enter on a selected agent opens chat mode with that agent
- Discover agents from both agent directories
- Show agent name and description in the list

**Non-Goals:**
- Agent configuration/editing from the TUI
- Running agents non-interactively from this tab (that's what schedules do)

## Decisions

### 1. Agent discovery at TUI startup

Scan `~/.kiro/agents/*.json` and vault `.kiro/agents/*.json` on refresh. Parse `name` and `description` fields from each JSON file. Deduplicate by name (vault-local wins).

### 2. Chat mode tracks active agent name

Store the agent name when entering chat so the header shows "Chat: good-morning" instead of generic "Weekly Review Chat".

### 3. Tab position

Agents tab goes at position 6 (after Stats). Key `6` switches to it.

## Risks / Trade-offs

- Minimal: adds one tab, reuses existing chat infrastructure. No protocol changes needed.
