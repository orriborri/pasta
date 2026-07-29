# Semantic Routing

## Purpose

Automatically route new tasks into matching initiative subfolders using vector similarity, linking execution (Tasks/) to planning (Roadmap/).

## Requirements

### Requirement: Auto-route new tasks to matching initiative
The system SHALL embed each new task's title and content, search for the most similar initiative in LanceDB, and move the task file into that initiative's subfolder when similarity exceeds 0.7.

#### Scenario: Task matches an initiative with high confidence
- **WHEN** a task "Add Prometheus scrape targets" is created in `Tasks/` root
- **AND** the initiative `Tasks/Monitoring.md` has description "Full observability stack: metrics, alerting, dashboards"
- **AND** cosine similarity between their embeddings is 0.82
- **THEN** the task file is moved to `Tasks/Monitoring/Add-prometheus-scrape-targets.md`

#### Scenario: Task does not match any initiative
- **WHEN** a task "Reply to Felix about Swedish publisher" is created in `Tasks/` root
- **AND** no initiative embedding has similarity > 0.7
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
- **THEN** all root-level `.md` files in `Tasks/` are evaluated against initiatives and moved if similarity > 0.7

### Requirement: Initiative detection
The system SHALL identify initiative files as any `.md` file in `Tasks/` root that has a corresponding subfolder with the same stem name (e.g., `Tasks/Monitoring.md` + `Tasks/Monitoring/`).

#### Scenario: Initiative with subfolder
- **WHEN** `Tasks/Monitoring.md` exists and `Tasks/Monitoring/` directory exists
- **THEN** `Monitoring` is recognized as a routable initiative target
