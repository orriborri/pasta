## Context

Pasta spawns kiro-cli agents as child processes. Some hang indefinitely when auth expires (kiro-cli tries to open a browser but runs headless). The scheduler holds `sched_state` lock for the entire agentic loop, acquiring `process` lock inside — fragile ordering. Logs accumulate forever.

## Goals / Non-Goals

**Goals:**
- Kill agents that exceed a timeout
- Report timeouts via the event system
- Clean up old logs automatically
- Fix ACP hardcoded path
- Reduce lock contention in scheduler

**Non-Goals:**
- Pre-validating kiro-cli auth tokens (outside our control)
- Multi-client server support
- Fan-out dependency model changes

## Decisions

### 1. Timeout via `tokio::time::timeout` wrapping `child.wait()`

In `spawn_agent`, the spawned task becomes:
```rust
tokio::spawn(async move {
    let timeout_dur = Duration::from_secs(timeout_minutes * 60);
    match tokio::time::timeout(timeout_dur, child.wait()).await {
        Ok(_) => {} // normal exit
        Err(_) => {
            child.kill().await.ok();
            log(&agent_name, "TIMEOUT — killed");
        }
    }
})
```

The timeout value comes from `config::get().general.agent_timeout_minutes`.

### 2. Log cleanup in vault_manager::run()

Add a `cleanup_old_logs()` step that removes `.log` files from `./logs/` older than 7 days. Runs once per vault-maintenance cycle (daily).

### 3. Narrow scheduler lock scope

Instead of holding `sched_state` for the entire `run_agentic_schedules` loop, collect the decisions (which agents to run) under the lock, then release it before spawning. This prevents lock-ordering issues.

### 4. ACP current_dir from config

Replace `"/home/orre/Obsidian/Readpeak"` with `pasta_common::vault::vault_path()`.

## Risks / Trade-offs

- **[Timeout too aggressive]** → 10 minutes is generous for inbox-processor (normally 30-40s). Configurable if needed.
- **[Kill mid-write]** → Agent killed mid-file-write could corrupt a markdown file. Acceptable since vault files are trivially recoverable from git/backups.
- **[Log cleanup deletes debug info]** → 7 days is enough to investigate issues. Old logs are noise.
