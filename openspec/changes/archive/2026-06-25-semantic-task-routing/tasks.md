## 1. Initiative Setup

- [x] 1.1 Move `Roadmap/` initiative content into `Tasks/` — create `Tasks/Monitoring.md`, `Tasks/Pipelines.md`, `Tasks/Data Platform.md`, `Tasks/Security & Backup.md` with `roadmap:` links back to `Roadmap/`
- [x] 1.2 Create corresponding subfolders `Tasks/Monitoring/`, `Tasks/Pipelines/`, etc.
- [x] 1.3 Move existing Roadmap execution tasks into the new `Tasks/<initiative>/` subfolders

## 2. Routing Logic

- [x] 2.1 Add `route_new_tasks()` to `crates/backend/src/fetchers/vault_manager.rs` — scans `Tasks/` root for unrouted `.md` files, embeds title, searches LanceDB for closest initiative, moves if similarity > 0.7
- [x] 2.2 Add `find_initiatives()` helper that detects initiative files (has matching subfolder)
- [x] 2.3 Call `route_new_tasks()` from `vault_manager::run()`

## 3. CLI Batch Command

- [x] 3.1 Add `--route-tasks` flag to `crates/backend/src/cli.rs` that runs `route_new_tasks()` for all existing root-level tasks
- [x] 3.2 Add `--route-tasks` to also auto-create initiative files + subfolders from `Roadmap/` if they don't exist in `Tasks/`

## 4. Config Update

- [x] 4.1 Update inbox-processor prompt in `~/.kiro/pasta.toml` to note that routing happens automatically after task creation
