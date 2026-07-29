use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    pub ts: DateTime<Local>,
    pub agent: String,
    pub hook: String,
}

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

pub fn history_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".kiro/run-history.jsonl")
}

pub fn load_history() -> Vec<RunEntry> {
    let path = history_path();
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
