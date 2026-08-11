use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Wire type for sending agent schedule info to TUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub agent: String,
    pub prompt: String,
    pub interval_minutes: u64,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub last_run: Option<DateTime<Local>>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

pub const KNOWN_NATIVE_SCHEDULES: &[&str] = &[
    "gitlab", "linear", "slack", "gmail", "calendar", "vault-maintenance", "trello",
];
