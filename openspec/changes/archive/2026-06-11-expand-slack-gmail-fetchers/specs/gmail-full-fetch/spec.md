## ADDED Requirements

### Requirement: Fetch all unread actionable emails
The Gmail fetcher SHALL retrieve all unread emails excluding promotions and social categories, without a time window restriction.

#### Scenario: Unread emails older than 1 day
- **WHEN** the fetcher runs
- **THEN** emails older than 1 day that are unread and not in promotions/social categories are included in the feed

#### Scenario: Feed output remains manageable
- **WHEN** there are many unread emails
- **THEN** results are capped (e.g., 50 threads max) to keep the feed concise
