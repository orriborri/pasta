## Context

The pasta-backend scheduler is a single `spawn()` function with a 6-second tick loop that evaluates both native fetchers (hardcoded Rust functions) and agentic schedules (CLI agent invocations from `schedules.json`). The TUI already has a `NativeSchedule` struct in IPC and renders a combined table. There's an existing `RunNow { index }` command for agents and `ForceFetch` for triggering all native fetchers at once, but no way to re-run a single native schedule.

## Goals / Non-Goals

**Goals:**
- Clean separation between native and agentic schedule evaluation in `scheduler.rs`
- Native schedule intervals configurable via TOML without recompilation
- Any schedule (native or agentic) can be manually re-triggered from TUI
- TUI renders two visually distinct sections
- Re-run is immediate, resets timer, skips if already running

**Non-Goals:**
- Changing what the native fetchers do internally
- Adding new native schedules or agents
- Changing the IPC protocol format (still JSON lines over Unix socket)
- Persistence of native schedule last-run across daemon restarts (keep in-memory as today)

## Decisions

### 1. Native schedule config in TOML

Create `~/.kiro/native-schedules.toml`:
```toml
[gitlab]
interval_minutes = 60
enabled = true

[linear]
interval_minutes = 60
enabled = true

[slack]
interval_minutes = 60
enabled = true

[gmail]
interval_minutes = 60
enabled = true

[calendar]
interval_minutes = 60
enabled = true

[vault-maintenance]
interval_minutes = 1440
enabled = true

[history-sync]
interval_minutes = 1440
enabled = true
```

Loaded once at daemon startup, no hot-reload needed (restart daemon to pick up changes). Generate default file if missing.

**Rationale**: TOML is consistent with other pasta config. Hot-reload adds complexity for little gain — schedule changes are rare.

### 2. Split scheduler into two functions

```rust
// scheduler.rs
pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let config = load_native_config();
        let mut ticker = interval(Duration::from_secs(6));
        loop {
            ticker.tick().await;
            run_native_schedules(&state, &config).await;
            run_agentic_schedules(&state).await;
        }
    });
}
```

Each function is self-contained. Native schedules use a registry mapping name → async function. Agentic schedules use `schedules.json` as today.

**Rationale**: Single responsibility. Each function can be reasoned about independently.

### 3. Native schedule registry pattern

```rust
struct NativeEntry {
    name: &'static str,
    run: fn(AppState) -> Pin<Box<dyn Future<Output = ()> + Send>>,
}
```

Maps config names to Rust functions. The scheduler iterates the registry, checks config (enabled + interval), checks last-run timestamp, and fires.

**Rationale**: Avoids a giant match statement. Adding a new native schedule = one registry entry.

### 4. RerunNative IPC command

Add `RerunNative { name: String }` to `Command` enum. Backend handles it by:
1. Check if that native schedule is currently running → if yes, send `Flash { "already running" }`
2. Reset its timer (`last_run = None` so next tick fires it)
3. Send `Flash { "↻ {name} triggered" }`

For agentic re-run, the existing `RunNow { index }` already works — just needs the "skip if running" check and flash feedback.

**Rationale**: Separate command for native vs agentic because they're different mechanisms (function call vs CLI spawn). Simple timer reset means the next 6s tick picks it up naturally — no special "run now" path needed.

### 5. TUI two-section layout

```
─── Native ─────────────────────────────────────────────
 Name              Freq     Next       Last Run
 gitlab            60m      in 59m     13:25
 linear            60m      in 59m     13:25

─── Agents ─────────────────────────────────────────────
 Agent             Trigger  Next       Last Run    Prompt
 inbox-processor   dep      on-dep     12:45       Process inbox...
 daily-writer      dep      on-dep     12:46       Create or update...
```

Selection spans both sections. `r` key sends `RerunNative` or `RunNow` depending on which section the cursor is in. A visual divider row separates the two sections.

**Rationale**: Single list with cursor is simpler than two separate widgets. The divider makes the distinction clear.

### 6. "Running" tracking for native schedules

Add a `HashSet<String>` to `SchedulerState` for currently-running native schedule names. Set on fire, clear on completion. Used by re-run skip check.

**Rationale**: Native schedules are spawned as tokio tasks. Need to track which are in-flight to prevent double-runs.

## Risks / Trade-offs

- [Risk] Config file could have invalid names → Mitigation: validate against known registry at load, warn and skip unknown entries
- [Risk] Timer reset on re-run could cause a schedule to fire twice in quick succession if tick fires between reset and execution → Mitigation: set `last_run = Some(now)` after firing, same as today
- [Trade-off] No hot-reload of config means daemon restart required for interval changes — acceptable for rare config changes
- [Trade-off] Divider row in TUI takes up one line of space — worthwhile for clarity
