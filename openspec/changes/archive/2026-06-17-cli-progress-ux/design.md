## Context

Currently `pasta-backend --sync-debug` outputs interleaved tracing logs to stderr and a log file simultaneously. The output is noisy (memory reports every 10s, debug per-channel lines) and provides no visual structure. Users can't quickly see overall progress or identify which step is slow/stuck.

## Goals / Non-Goals

**Goals:**
- Clean, scannable terminal output showing all stages at a glance
- In-place updates (no scrolling wall of text)
- ETA and progress bar for the indexing stage
- Memory usage visible at all times
- Verbose debug logs still written to `./logs/tracing.log`

**Non-Goals:**
- Interactive TUI (no key input, no panels)
- Color themes or configuration
- Log rotation or log level CLI flags (can add later)

## Decisions

### 1. Use `indicatif` for terminal rendering

**Rationale:** It handles in-place line updates, progress bars, multi-line redraws, and terminal width detection. Well-maintained, zero unsafe, widely used in Rust CLI tools.

**Alternative:** Manual ANSI escape codes — fragile, no terminal detection, more code.

### 2. Tracing to file only, progress to stderr via indicatif

**Rationale:** Separating concerns — tracing is for debugging (structured, verbose), the progress display is for humans (concise, visual). Mixing them produces unreadable output.

**Structure:**
- `tracing_subscriber` with file-only layer (`./logs/tracing.log`)
- `indicatif::MultiProgress` for the terminal display
- Each syncer gets a line in the multi-progress that updates in place

### 3. Each syncer returns a `SyncResult` struct

```rust
struct SyncResult {
    items: usize,
    label: String, // e.g. "26,932 msgs" or "15 issues"
}
```

**Rationale:** Decouples display from sync logic. The orchestrator owns the display, syncers just report what they did.

### 4. Display layout

```
pasta-backend sync
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Slack       63 channels   26,932 msgs
✓ Gmail       14 threads
▸ Linear      fetching...                  47MB
○ Wiki
○ Repos
○ Indexing
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

During indexing, replace the indexing line with a progress bar:
```
▸ Indexing    32/120 files   ETA 45s       285MB
  ████████░░░░░░░░░░░░░░░░░  27%
```

Final summary line:
```
Done in 3m 12s · Peak memory: 285MB
```

## Risks / Trade-offs

- **[Risk] Terminal without ANSI support** → `indicatif` degrades gracefully to plain text
- **[Risk] Long-running syncer with no progress** (e.g., gmail waiting for API) → Show elapsed time spinner on active line
- **[Trade-off] Removing stderr tracing** means you must `tail -f ./logs/tracing.log` separately for debugging — acceptable since that's the intended workflow
