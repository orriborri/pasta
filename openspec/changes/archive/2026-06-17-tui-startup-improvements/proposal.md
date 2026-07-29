## Why

The TUI startup experience had several issues: stale sockets caused connection failures, multiple orphaned backend processes could accumulate, the good-morning agent fired before fetchers completed (missing context), the Today tab rendered raw markdown instead of a usable interface, and native fetchers + history sync duplicated API calls on startup.

## What Changes

- Robust backend connection logic that handles stale sockets and orphaned processes
- Good-morning agent deferred until FetchComplete event (data is ready)
- Today tab as a structured native view (meetings, tasks with priorities, waiting)
- Task completion directly from the TUI
- Skip history sync when native fetchers already ran on the same tick

## Capabilities

### New Capabilities

- Toggle task completion from Today tab with `x` key
- Navigate tasks with j/k in Today tab
- Detect and offer to kill orphaned backend processes on startup
- Structured Today view with meetings, prioritized tasks, and waiting sections

### Changed Capabilities

- Backend connection retries with stale socket cleanup
- Good-morning waits for fetch data before starting
- History sync skips when native fetchers just ran (avoids duplicate API calls)
