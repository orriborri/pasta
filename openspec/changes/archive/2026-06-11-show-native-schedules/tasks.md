## 1. Common types

- [x] 1.1 Add `NativeSchedule` struct to `crates/common/src/ipc.rs` (name, interval_minutes, last_run)
- [x] 1.2 Add `native_schedules: Vec<NativeSchedule>` field to the `State` event variant

## 2. Backend

- [x] 2.1 Move `last_native_fetch` and `last_vault_maintenance` into `AppState`
- [x] 2.2 Build native schedule list in `build_state_snapshot()` and include in State event

## 3. TUI

- [x] 3.1 Store `native_schedules` in `App` struct, update from State event
- [x] 3.2 Render native schedules in `draw_schedule()` above user schedules with dimmed/system style
- [x] 3.3 Offset selection index so native rows are not selectable (skip actions for native indices)
