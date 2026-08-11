## 1. Task tracking lives in the Kiro spec

Implementation is executed by Ralph, which reads and updates
`.kiro/specs/remove-accreted-architecture/tasks.md`. That file is the single
source of truth for task state; duplicating checkboxes here would give two
answers to "what is done".

- [ ] 1.1 Execute `.kiro/specs/remove-accreted-architecture/tasks.md` (19 tasks, phases 1-19)
- [ ] 1.2 Archive this change once those tasks are complete, promoting the delta specs in `specs/` into `openspec/specs/`

## 2. What this change owns

The artifacts that stay authoritative here:

- `proposal.md` — why the change exists, what it removes, its impact
- `design.md` — decisions and alternatives considered
- `specs/vault-layout/spec.md`, `specs/task-visibility/spec.md` — new capabilities
- `specs/kb-api/spec.md`, `specs/search-filtering/spec.md`, `specs/state-persistence/spec.md` — requirement deltas to be promoted on archive

The Kiro spec restates these as EARS acceptance criteria for the executor. If a
requirement changes, change it here first, then reflect it in
`.kiro/specs/remove-accreted-architecture/requirements.md`.
