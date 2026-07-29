## Why

Tasks arrive untagged in `Tasks/` and the user must manually decide which roadmap initiative they belong to. The LanceDB vector index already contains embeddings of both tasks and initiative descriptions — we can use semantic similarity to automatically route tasks to the correct initiative subfolder when they land.

## What Changes

- **Auto-routing on task creation**: When inbox-processor creates a task, a post-processing step embeds the task content and searches for the most similar initiative. If confidence is high (>0.7 similarity), the task file is moved into that initiative's subfolder automatically.
- **Batch routing for existing tasks**: A CLI command (`--route-tasks`) that processes all unorganized tasks at root level and moves them into matching initiative subfolders.
- **Initiative index**: Initiative files are kept indexed in LanceDB so similarity search works against them.

## Capabilities

### New Capabilities
- `semantic-routing`: Automatic task-to-initiative matching using vector similarity from LanceDB

### Modified Capabilities
- `state-persistence`: Initiative embeddings tracked for routing lookups

## Impact

- `crates/backend/src/fetchers/vault_manager.rs` — add `route_new_tasks()` function
- `crates/backend/src/history/indexer.rs` — expose a simple search-by-text helper for internal use
- `crates/backend/src/cli.rs` — add `--route-tasks` command
- `~/.kiro/pasta.toml` — inbox-processor prompt updated to mention routing happens automatically
- Folder structure: `Tasks/` gains initiative subfolders with tasks auto-moved into them
