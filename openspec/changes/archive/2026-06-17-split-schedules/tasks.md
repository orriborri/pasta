## 1. Native Schedule Config

- [x] 1.1 Create `NativeScheduleConfig` struct and TOML loader in `crates/common/src/data.rs`
- [x] 1.2 Add `load_native_config()` that reads `~/.kiro/native-schedules.toml` or generates defaults
- [x] 1.3 Validate config entries against known native schedule names, warn on unknown

## 2. Native Schedule Registry

- [x] 2.1 Define `NativeEntry` struct with name and async run function in `crates/backend/src/scheduler.rs`
- [x] 2.2 Create registry array mapping names to functions: gitlab, linear, slack, gmail, calendar, vault-maintenance, history-sync
- [x] 2.3 Add `running_native: HashSet<String>` to `SchedulerState` for tracking in-flight native schedules

## 3. Split Scheduler Loop

- [x] 3.1 Extract `run_native_schedules()` function that iterates registry, checks config + interval + running state
- [x] 3.2 Extract `run_agentic_schedules()` function with existing agent schedule logic
- [x] 3.3 Update `spawn()` to call both functions sequentially per tick
- [x] 3.4 Remove hardcoded native schedule logic (fetch cycle, vault maintenance) from the old monolithic loop
- [x] 3.5 Verify `cargo build` passes

## 4. RerunNative IPC Command

- [x] 4.1 Add `RerunNative { name: String }` variant to `Command` enum in `crates/common/src/ipc.rs`
- [x] 4.2 Handle `RerunNative` in `commands.rs`: check if running → flash, else reset timer → flash
- [x] 4.3 Update `RunNow` handler to check if agent already running → flash skip message

## 5. TUI Two-Section Display

- [x] 5.1 Update schedule tab rendering to show native section with header: Name, Freq, Next, Last Run
- [x] 5.2 Add divider row between native and agentic sections
- [x] 5.3 Render agentic section with header: Agent, Trigger, Next, Last Run, Prompt
- [x] 5.4 Make cursor selection span both sections seamlessly

## 6. TUI Re-run Key Binding

- [x] 6.1 Add `r` key handler in schedule tab that determines if selected row is native or agentic
- [x] 6.2 Send `RerunNative { name }` for native rows
- [x] 6.3 Send `RunNow { index }` for agentic rows
- [x] 6.4 Verify flash messages appear for both trigger and skip-if-running cases

## 7. Integration Verification

- [x] 7.1 Verify `cargo build --release` passes for all crates
- [x] 7.2 Test daemon starts and loads config (or generates default)
- [x] 7.3 Test re-run of a native schedule triggers it immediately
- [x] 7.4 Test re-run of an agentic schedule triggers it immediately
