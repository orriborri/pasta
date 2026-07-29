## Why

The Schedule tab only shows user-defined agent schedules from `schedules.json`. The built-in native schedules (gitlab, linear, slack, gmail, calendar fetchers every 60min; vault maintenance every 24h) are invisible to the user. You can't see when they last ran or when they'll run next.

## What Changes

- Expose native/built-in schedules in the backend state alongside user-defined schedules
- Display them in the Schedule tab (visually distinguished, e.g. dimmed or marked as "system")
- Show last-run and next-run timing for native schedules
- Native schedules are read-only (can't be deleted or edited by the user)

## Capabilities

### New Capabilities
- `native-schedule-visibility`: Display built-in native schedules (fetchers, vault maintenance) in the Schedule tab with their timing info

### Modified Capabilities

## Impact

- `crates/common/src/ipc.rs` — State event gains native schedule info
- `crates/backend/src/main.rs` — Track and expose native schedule state
- `crates/tui/src/ui.rs` — Render native schedules in Schedule tab (distinct from user schedules)
