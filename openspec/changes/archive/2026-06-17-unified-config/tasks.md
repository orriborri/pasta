## 1. Config Struct and Loader

- [x] 1.1 Define `AppConfig` struct in `crates/common/src/config.rs` with sections: general, schedules, agents, repos, binaries
- [x] 1.2 Implement `load_config()` that reads `~/.kiro/pasta.toml` or generates defaults
- [x] 1.3 Add `once_cell` dependency and create `config::get() -> &'static AppConfig` accessor
- [x] 1.4 Generate default config with inline comments when file is missing
- [x] 1.5 Handle parse errors gracefully (log + fallback to defaults)

## 2. Replace Vault Path Constant

- [x] 2.1 Change `VAULT_PATH` const to `pub fn vault_path() -> &'static str` reading from config
- [x] 2.2 Update all `VAULT_PATH` references to `vault_path()` calls
- [x] 2.3 Verify `cargo build` passes

## 3. Consolidate Native Schedules

- [x] 3.1 Remove `~/.kiro/native-schedules.toml` loader (`load_native_config` in data.rs)
- [x] 3.2 Update `scheduler.rs` to read native schedules from `config::get().schedules`
- [x] 3.3 Update `server.rs` state snapshot to use unified config

## 4. Consolidate Agentic Schedules

- [x] 4.1 Remove `load_schedules()` / `save_schedules()` from data.rs
- [x] 4.2 Update scheduler `run_agentic_schedules()` to read from `config::get().agents`
- [x] 4.3 Track `last_run` in-memory only (in SchedulerState)
- [x] 4.4 Remove `AddSchedule` and `RemoveSchedule` from IPC Command enum
- [x] 4.5 Remove `a`/`d` key handlers from TUI schedule tab
- [x] 4.6 Add flash message when user presses `a`: "Edit ~/.kiro/pasta.toml to add agents"

## 5. Consolidate Repo Config

- [x] 5.1 Remove `history/config.rs` loader
- [x] 5.2 Update `sync_repos.rs` and `sync_git_log.rs` to read from `config::get().repos`

## 6. Consolidate Binary Paths

- [x] 6.1 Update `resolve_binary()` in fetchers to check `config::get().binaries` first
- [x] 6.2 Empty string = auto-detect from PATH (existing behavior)

## 7. Cleanup and Verification

- [x] 7.1 Remove old config files: delete `native_config_path()`, `schedule_path()`, repo-index generation
- [x] 7.2 Remove `toml` dependency from `pasta-common` if no longer needed there (move to backend or keep shared)
- [x] 7.3 Verify `cargo build --release` passes for all crates
- [x] 7.4 Test daemon starts and generates default config
- [x] 7.5 Test `--sync-once` works with unified config
