# Semantic Routing

## Purpose

Automatically route new tasks into matching initiative subfolders using the
knowledge-base hybrid search, linking execution (Tasks/) to planning (Roadmap/).

## Requirements

### Requirement: Auto-route new tasks to matching initiative
The system SHALL query the knowledge-base hybrid search for each new task's
title (scoped to `source=vault`, top 3 hits) and move the task file into an
initiative's subfolder when one of those hits names a known initiative
(case-insensitive substring match on the hit's path or content). No fixed
similarity threshold is applied — matching is by an initiative name appearing in
the retrieved results.

#### Scenario: Task matches an initiative
- **WHEN** a task "Add Prometheus scrape targets" is created in `Tasks/` root
- **AND** a top-3 hybrid-search hit for that title references the `Monitoring` initiative
- **THEN** the task file is moved to `Tasks/Monitoring/Add-prometheus-scrape-targets.md`

#### Scenario: Task does not match any initiative
- **WHEN** a task "Reply to Felix about Swedish publisher" is created in `Tasks/` root
- **AND** none of the top-3 hits name a known initiative
- **THEN** the task file stays at `Tasks/` root (appears in Uncategorized on Kanban)

#### Scenario: Task already in a subfolder is not re-routed
- **WHEN** vault-maintenance runs and finds `Tasks/Monitoring/EKS-alerting.md`
- **THEN** it is skipped — only root-level tasks are candidates for routing

### Requirement: Initiatives link back to roadmap
Initiative files in `Tasks/` SHALL contain a `roadmap:` frontmatter field linking to the corresponding `Roadmap/` planning file. This connects execution (Tasks/) to planning (Roadmap/).

#### Scenario: Initiative with roadmap link
- **WHEN** `Tasks/Monitoring.md` has `roadmap: "[[Roadmap/Monitoring]]"`
- **THEN** navigating from the task layer to the roadmap Gantt view is one click away

#### Scenario: Auto-create initiative when roadmap exists
- **WHEN** a roadmap file `Roadmap/Monitoring.md` exists but `Tasks/Monitoring/` does not
- **AND** `--route-tasks` is run
- **THEN** the system creates `Tasks/Monitoring.md` (with `roadmap: "[[Roadmap/Monitoring]]"`) and `Tasks/Monitoring/` subfolder

### Requirement: Batch route existing tasks via CLI
The system SHALL provide a `--route-tasks` CLI flag that processes all existing root-level tasks in `Tasks/` and routes them to matching initiative subfolders.

#### Scenario: Initial migration
- **WHEN** user runs `pasta-backend --route-tasks`
- **THEN** all root-level `.md` files in `Tasks/` are evaluated against initiatives and moved when a top-3 hybrid-search hit names a matching initiative

### Requirement: Initiative detection
The system SHALL identify initiative files as any `.md` file in `Tasks/` root that has a corresponding subfolder with the same stem name (e.g., `Tasks/Monitoring.md` + `Tasks/Monitoring/`).

#### Scenario: Initiative with subfolder
- **WHEN** `Tasks/Monitoring.md` exists and `Tasks/Monitoring/` directory exists
- **THEN** `Monitoring` is recognized as a routable initiative target
