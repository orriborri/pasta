## ADDED Requirements

### Requirement: GitLab fetcher ingests merge requests via the glab CLI
The system SHALL provide a GitLab fetcher that retrieves merge requests for configured repositories using the `glab` CLI, deriving the GitLab project path from each repository's `origin` remote. It SHALL emit records with `Source::Gitlab` and `Kind::Merge`.

#### Scenario: Forward MR fetch
- **WHEN** the GitLab fetcher runs `fetch_updates`
- **THEN** it lists merge requests updated since the stored cursor for each configured repo
- **AND** emits one record per MR with title, author, state, source branch, and web URL

#### Scenario: glab unavailable or unauthenticated
- **WHEN** the `glab` CLI is missing or not authenticated
- **THEN** the fetcher returns an empty result and logs a warning
- **AND** the overall fetch cycle continues for other sources

### Requirement: MR records carry a populated URL and author identity
Each merge-request record SHALL set `url` to the MR's web URL and `author` to the MR author, so the record is directly linkable and attributable.

#### Scenario: MR record fields
- **WHEN** an MR record is produced
- **THEN** its `url` is the GitLab MR web URL
- **AND** its `author` is the MR author's username, resolvable to a canonical person via the registry

### Requirement: GitLab fetcher resolves merge requests by reference
The GitLab fetcher SHALL declare `mr` in its `resolves()` set and implement `fetch_targets` to retrieve a merge request when the reference carries a project and IID.

#### Scenario: Resolve an MR referenced from a Linear comment
- **WHEN** `fetch_targets` receives `EntityRef { kind: Mr, id: "2935", project: Some("group/proj") }`
- **THEN** the fetcher retrieves MR !2935 for that project and emits its record

### Requirement: Linear references are extracted from MR branch and title
The system SHALL extract Linear issue identifiers from a merge request's source branch and title, normalizing lowercase forms (e.g. `user/ab-123-slug`) to the canonical uppercase entity `linear:AB-123`.

#### Scenario: Linear ID only in branch name
- **WHEN** an MR has source branch `oscar/ab-123-fix-login` and no ticket mention in the title
- **THEN** the record's entities include `linear:AB-123`
