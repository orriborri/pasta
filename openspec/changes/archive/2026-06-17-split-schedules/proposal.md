## Why

The scheduler currently mixes two distinct concerns in one tick loop: native fetchers (hardcoded Rust functions with fixed intervals) and agentic schedules (user-defined CLI agent invocations with prompts and dependency chains). This makes it hard to reason about, configure, or manually re-run individual schedules. Native schedule intervals are hardcoded, requiring recompilation to change. The TUI shows them in a single flat table despite having different semantics.

## What Changes

- Split `scheduler.rs` into two clear subsystems: native schedules (data fetchers, vault maintenance, history sync) and agentic schedules (inbox-processor, daily-writer, etc.)
- Move native schedule intervals to a `native-schedules.toml` config file so they can be tuned without recompiling
- Add a `RerunNative { name }` IPC command so individual native schedules can be triggered from the TUI
- Make the existing `RunNow { index }` command work for agentic schedules re-run
- Update TUI to render two separate sections with appropriate columns
- Add `r` key binding to trigger re-run on the selected schedule (native or agentic)
- Re-run triggers immediately and resets the timer; skips if already running (flash message)

## Capabilities

### New Capabilities
- `native-schedule-config`: TOML-based configuration for native schedule intervals and enabled state
- `schedule-rerun`: Ability to manually re-run any schedule (native or agentic) from TUI with immediate trigger, timer reset, and skip-if-running semantics

### Modified Capabilities

## Impact

- `crates/backend/src/scheduler.rs` — split into native + agentic loops
- `crates/backend/src/commands.rs` — handle new `RerunNative` command
- `crates/common/src/ipc.rs` — add `RerunNative` command variant
- `crates/common/src/data.rs` — may need config loading for native schedules
- `crates/tui/src/ui.rs` — two-section schedule display, `r` key binding
- New file: config/native-schedules.toml
