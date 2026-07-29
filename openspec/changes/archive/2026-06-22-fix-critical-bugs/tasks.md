## 1. Daemon Singleton

- [x] 1.1 Add `flock`-based lock acquisition in `main.rs` before socket bind — open `~/.kiro/pasta.lock`, attempt `LOCK_EX | LOCK_NB`, exit with error message on failure
- [x] 1.2 Keep the lock file descriptor alive for daemon lifetime (store in a variable that is not dropped)

## 2. Fix persist_hashes

- [x] 2.1 In `indexer.rs`, change `persist_hashes` to call `state::load_state()` from disk, merge `new_hashes` into the fresh state, then `save_state`

## 3. Fix frontmatter insertion

- [x] 3.1 In `feed_history.rs` `update_task_dependencies`, replace the broken `split_at` logic with: find content between first `---\n` and second `\n---`, insert or replace `dependsOn:` line before the closing `---`

## 4. Remove hardcoded paths

- [x] 4.1 In `history/sync_slack.rs`, replace `const SLACK_API` with a call to `crate::fetchers::resolve_binary("slack-api", "SLACK_API_PATH", "slack-api")`
- [x] 4.2 In `acp.rs`, replace hardcoded kiro-cli path with `crate::fetchers::resolve_binary("kiro-cli", "KIRO_CLI_PATH", "kiro-cli")`

## 5. Fix mr_status formatting

- [x] 5.1 In `fetchers/gitlab.rs`, rewrite `mr_status` to collect status parts and join with `, ` instead of the broken paren-concatenation logic
