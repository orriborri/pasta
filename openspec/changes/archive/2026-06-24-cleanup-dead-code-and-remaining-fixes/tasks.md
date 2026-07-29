## 1. Dead Code Removal

- [x] 1.1 Remove `pub fn fetch()` from `crates/backend/src/fetchers/gitlab.rs`
- [x] 1.2 Remove `pub fn fetch()` from `crates/backend/src/fetchers/gmail.rs`
- [x] 1.3 Remove `pub fn fetch()` from `crates/backend/src/fetchers/linear.rs`
- [x] 1.4 Remove `pub fn fetch()` from `crates/backend/src/fetchers/slack.rs`
- [x] 1.5 Remove `async fn embed_openai` and `async fn embed_ollama` standalone functions from `crates/backend/src/history/embedder.rs`
- [x] 1.6 Delete `crates/backend/src/history/simple_index.rs` and remove its `pub mod simple_index;` declaration from `mod.rs`
- [x] 1.7 Remove `SIMILARITY_THRESHOLD` constant from `crates/backend/src/history/vault_organize.rs`
- [x] 1.8 Remove `deps_to_clear` field from `SpawnDecision` struct in `crates/backend/src/scheduler.rs`

## 2. Search Pre-Limit Filtering

- [x] 2.1 In `crates/backend/src/history/indexer.rs` `search()`, build a LanceDB `where` filter string from source/participant/after params and apply it before `.limit()`
- [x] 2.2 In `crates/mcp/src/main.rs` `do_search()`, apply the same filter approach (build where clause, remove post-retrieval filtering)

## 3. Git Log Parsing Fix

- [x] 3.1 In `crates/backend/src/history/sync_git_log.rs`, change `git log --format` to use `%x00` field separators and `%x01` record separators, remove `--stat`
- [x] 3.2 Rewrite `parse_log_entries` to split on `\x01` for records and `\x00` for fields

## 4. Repair Links Search Quality

- [x] 4.1 In `crates/backend/src/history/vault_organize.rs` `repair_links`, strip frontmatter before taking content for the search query; fall back to document title if content is too short

## 5. Embedding Documentation

- [x] 5.1 Add a doc comment to `embedder.rs` `OllamaProvider::embed` explaining the dimension mismatch limitation and that mixed-mode indices degrade search quality
