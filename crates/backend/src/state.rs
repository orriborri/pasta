use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Local};
use tokio::sync::{mpsc, Mutex};

use pasta_common::ipc::Event;

pub type EventTx = mpsc::UnboundedSender<Event>;

/// Shared daemon state. Each field is independently locked to reduce contention.
/// Lock ordering: process → connection (never acquire connection while holding process).
#[derive(Clone)]
pub struct AppState {
    pub process: Arc<Mutex<ProcessState>>,
    pub connection: Arc<Mutex<ConnectionState>>,
    pub scheduler: Arc<Mutex<SchedulerState>>,
}

pub struct ProcessState {
    pub running: HashMap<usize, tokio::task::JoinHandle<()>>,
    pub completed: HashSet<String>,
}

pub struct ConnectionState {
    pub event_tx: Option<EventTx>,
}

pub struct SchedulerState {
    pub last_run: HashMap<String, DateTime<Local>>,
    pub running_native: HashSet<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(ProcessState {
                running: HashMap::new(),
                completed: HashSet::new(),
            })),
            connection: Arc::new(Mutex::new(ConnectionState {
                event_tx: None,
            })),
            scheduler: Arc::new(Mutex::new(SchedulerState {
                last_run: HashMap::new(),
                running_native: HashSet::new(),
            })),
        }
    }
}
