pub const KNOWN_NATIVE_SCHEDULES: &[&str] = &[
    "gitlab", "linear", "slack", "gmail", "calendar", "vault-maintenance", "trello",
];

/// The native fetchers that run together as the single "fetch-cycle" group
/// (a subset of `KNOWN_NATIVE_SCHEDULES`). Shared so the scheduler, command
/// handler, status view, and default config all agree on group membership.
pub const FETCH_GROUP_NAMES: [&str; 5] = ["gitlab", "linear", "slack", "gmail", "calendar"];
