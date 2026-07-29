use crate::data::Schedule;
use crate::vault::{Feed, Person, Task};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSchedule {
    pub name: String,
    pub interval_minutes: u64,
    pub last_run: Option<DateTime<Local>>,
    pub running: bool,
}

pub fn socket_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".pasta/pasta.sock")
}

/// TUI → Backend commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    RunNow { index: usize },
    RerunNative { name: String },
    ForceFetch,
    Backfill,
    Organize,
    ChatStart { agent: String },
    ChatPrompt { text: String },
    ChatStop,
    /// Request full state
    Sync,
    /// Search the history index
    Search { query: String, source: Option<String>, participant: Option<String>, after: Option<String>, limit: Option<usize> },
}

/// Backend → TUI events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "evt", rename_all = "snake_case")]
pub enum Event {
    State {
        schedules: Vec<Schedule>,
        native_schedules: Vec<NativeSchedule>,
        running: Vec<usize>,
        tasks: Vec<Task>,
        feeds: Vec<Feed>,
        people: Vec<Person>,
    },
    AgentStarted { index: usize, agent: String },
    AgentFinished { index: usize, agent: String },
    FetchComplete,
    Chat { chat_type: ChatEventType },
    Flash { message: String },
    SearchResults { results: Vec<SearchResultItem> },
    SyncProgress { stage: String, done: usize, total: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub path: String,
    pub source: String,
    pub participants: String,
    pub date: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEventType {
    Text { text: String },
    ToolCall { name: String, status: String },
    TurnEnd,
    Error { message: String },
}
