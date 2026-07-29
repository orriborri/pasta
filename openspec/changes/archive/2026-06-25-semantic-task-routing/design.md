## Context

The vault has initiative files (`Tasks/Monitoring.md`, `Tasks/Pipelines.md`, etc.) describing high-level goals, and task files created by the inbox-processor. LanceDB already indexes all vault content with embeddings. We can match new tasks to initiatives via cosine similarity.

## Goals / Non-Goals

**Goals:**
- New tasks automatically moved to initiative subfolders when similarity > threshold
- Existing unrouted tasks can be batch-processed
- User can override by moving files manually (no re-routing of already-placed tasks)

**Non-Goals:**
- Creating new initiatives automatically (user creates those)
- Routing tasks that are already in a subfolder
- Real-time streaming (vault-maintenance cycle is fast enough)

## Decisions

### 1. Routing logic in vault_manager (runs during vault-maintenance)

**Decision**: Add `route_new_tasks()` to vault-maintenance. It scans `Tasks/` root for files without a subfolder, embeds each title, searches LanceDB for the closest initiative, and moves if similarity > 0.7.

**Rationale**: Runs every maintenance cycle (daily). No new async infrastructure needed. Reuses existing embedder.

### 2. Initiative detection by frontmatter marker

**Decision**: Initiative files are identified by being `.md` files directly in `Tasks/` that have no `status: todo/done` (they're parent rollup files, not tasks). Or simpler: look for files that Task Gantt uses as parents (files with subfolders named the same).

**Actually simplest**: Any `.md` file in `Tasks/` that has a corresponding subfolder with the same stem name is an initiative.

### 3. Similarity threshold: 0.7

Below 0.7: task stays at root (Uncategorized). User routes manually.
Above 0.7: auto-move to initiative subfolder.

### 4. Skip already-routed tasks

Only process tasks at `Tasks/*.md` root level. Tasks already in `Tasks/<initiative>/` are never re-routed.

### 5. CLI batch command for initial migration

`pasta-backend --route-tasks` processes all existing root-level tasks once. Good for the initial setup.

## Risks / Trade-offs

- [Wrong routing] → 0.7 is conservative. If a task gets misrouted, user drags it back. Vault-maintenance won't re-route it once in a subfolder.
- [Embedding cost] → Each new task needs one embedding call. At ~5 tasks/day this is negligible.
