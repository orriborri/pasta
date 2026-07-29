## Context

Pasta is a Rust daemon (`pasta-backend`) that schedules fetchers and agent processes. It communicates with a TUI over a Unix socket. The daemon currently has no mechanism to prevent multiple instances, leading to duplicate work. Several internal functions have logic errors that cause data corruption or incorrect output.

## Goals / Non-Goals

**Goals:**
- Prevent two daemon instances from running concurrently
- Ensure `persist_hashes` never overwrites state written by concurrent operations
- Fix frontmatter manipulation so it never corrupts markdown files
- Eliminate hardcoded paths in favor of the existing `BinaryConfig` system
- Fix `mr_status` output formatting

**Non-Goals:**
- Multi-client support (server.rs single-client design is intentional for now)
- Refactoring the busy-wait `run_cmd` (functional, just suboptimal)
- Cleaning up dead code in `embedder.rs`

## Decisions

### 1. PID-file lock for daemon singleton

Use `flock()` on a lock file at `~/.kiro/pasta.lock`. The daemon opens the file with `O_CREAT | O_RDWR`, attempts a non-blocking exclusive lock (`flock(fd, LOCK_EX | LOCK_NB)`). If it fails, print an error and exit. The lock is held for the lifetime of the process and automatically released on crash/kill.

**Why not PID-file alone?** PID files go stale on crashes. `flock()` is kernel-managed and always released.

**Alternative considered:** Advisory lock on the socket file itself. Rejected because the socket is deleted/recreated on startup, which would break the lock.

### 2. Fix `persist_hashes` — load from disk

Change `persist_hashes` to call `state::load_state()` from disk, merge new hashes into that fresh state, then save. This ensures concurrent updates from other operations (e.g., `run_sync`) are preserved.

**Trade-off:** Slight overhead of re-reading the state file per batch. Acceptable since it's a small JSON and writes happen every ~10 chunks.

### 3. Fix frontmatter insertion in `update_task_dependencies`

Replace the broken `split_at` approach with a simple regex-free strategy: find the second `---` line, insert `dependsOn: [...]` before it on its own line. If `dependsOn:` already exists, replace it.

### 4. Use `resolve_binary` for hardcoded paths

`sync_slack.rs` has `const SLACK_API: &str = "/home/orre/.local/bin/slack-api"`. Replace with a call to `crate::fetchers::resolve_binary("slack-api", "SLACK_API_PATH", "slack-api")`.

`acp.rs` hardcodes `/home/orre/.nix-profile/bin/kiro-cli`. Replace with `crate::fetchers::resolve_binary("kiro-cli", "KIRO_CLI_PATH", "kiro-cli")`.

### 5. Fix `mr_status` formatting

Replace the confused join/concat logic with a simple approach: collect status parts into a Vec, join with `, `.

## Risks / Trade-offs

- **[flock portability]** → `flock()` is Linux/macOS only. Not a concern since pasta targets Linux.
- **[persist_hashes race]** → Two writers could still race on the same JSON file. Mitigated by the fact that only one indexer runs at a time (enforced by scheduler). If this becomes a real problem later, add file locking.
- **[frontmatter edge cases]** → Files without proper `---` delimiters are skipped (no-op). This is the existing behavior.
