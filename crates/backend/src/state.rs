use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Local};
use tokio::sync::{mpsc, Mutex};

use pasta_common::ipc::Event;

pub type EventTx = mpsc::UnboundedSender<Event>;

/// Shared daemon state. Each field is independently locked to reduce contention.
#[derive(Clone)]
pub struct AppState {
    pub connection: Arc<Mutex<ConnectionState>>,
    pub scheduler: Arc<Mutex<SchedulerState>>,
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
