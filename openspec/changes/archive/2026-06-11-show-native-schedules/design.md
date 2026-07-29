## Context

Native schedules (fetchers + vault maintenance) are hardcoded in the backend scheduler loop. Their timing state (`last_native_fetch`, `last_vault_maintenance`) lives in local variables inside the scheduler task. The TUI Schedule tab only renders `Vec<Schedule>` from the IPC state.

## Goals / Non-Goals

**Goals:**
- Show native schedules in the Schedule tab with name, interval, last run, and next run
- Visually distinguish native (system) from user-defined schedules
- Keep native schedules read-only in the TUI (no delete/edit)

**Non-Goals:**
- Making native schedules configurable
- Changing fetch intervals dynamically

## Decisions

### 1. Add `NativeSchedule` struct to common

```rust
pub struct NativeSchedule {
    pub name: String,
    pub interval_minutes: u64,
    pub last_run: Option<DateTime<Local>>,
}
```

Sent as part of the `State` event. Simple data-only struct, no behavior.

### 2. Backend tracks native state in shared struct

Move `last_native_fetch` and `last_vault_maintenance` into `AppState` so they can be included in state snapshots.

### 3. TUI renders native schedules above user schedules

Show them with a dimmed style and a "system" marker. Skip selection/actions for native schedule rows.

## Risks / Trade-offs

- Minimal complexity: just a new struct + a few fields in state. No architectural risk.
