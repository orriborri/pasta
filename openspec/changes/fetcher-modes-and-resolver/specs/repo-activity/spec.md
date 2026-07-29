## ADDED Requirements

### Requirement: Commit records capture changed files
The git fetcher SHALL capture the list of files changed by each commit (with change status: added, modified, deleted, renamed) and include it in the commit record's content.

#### Scenario: Commit touching multiple files
- **WHEN** a commit modifies two files and deletes one
- **THEN** the commit record's content includes a changed-files section listing the three files with their statuses

#### Scenario: Commit with a multi-paragraph body
- **WHEN** a commit message body contains blank lines
- **THEN** the body and the changed-files list are captured without conflating one for the other

### Requirement: Commit records carry a repository URL
The git fetcher SHALL populate each commit record's `url` with a web link derived from the repository's `origin` remote, normalizing SSH and HTTPS remote forms to a canonical commit URL.

#### Scenario: SSH remote normalized
- **WHEN** a repo's origin remote is `git@host:group/proj.git`
- **THEN** a commit record's url is `https://host/group/proj/-/commit/<hash>`

#### Scenario: Repository without a usable remote
- **WHEN** a repo has no `origin` remote
- **THEN** the commit record's url is left empty and the commit is still recorded

### Requirement: Commits are linked to project and Linear prefix
The enrichment stage SHALL resolve a commit's repository to a project via the entity registry and tag the record with the project and its Linear prefix, so commits link to their Linear work area even without an explicit ticket mention.

#### Scenario: Repo mapped to a project
- **WHEN** a commit comes from repo `mononode`
- **AND** a project file lists `mononode` under `gitlab_repos` with `linear_prefix: AB`
- **THEN** the commit record is tagged `project:<name>` and `linear-prefix:AB`

#### Scenario: Explicit ticket mention still links directly
- **WHEN** a commit message contains `AB-123`
- **THEN** the record's entities include `linear:AB-123` and it merges with the AB-123 record during cross-source dedup
