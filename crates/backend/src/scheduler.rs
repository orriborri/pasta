use pasta_common::config;
use pasta_common::ipc::Event;

use chrono::Local;
use std::future::Future;
use std::pin::Pin;
use tokio::time::{interval, Duration};

use crate::process::{cleanup_finished, spawn_agent};
use crate::state::AppState;
use crate::util::log;

type NativeFn = fn(AppState) -> Pin<Box<dyn Future<Output = ()> + Send>>;

struct NativeEntry {
    name: &'static str,
    run: NativeFn,
}

const REGISTRY: &[NativeEntry] = &[
    NativeEntry { name: "gitlab", run: run_fetch_cycle },
    NativeEntry { name: "linear", run: run_fetch_cycle },
    NativeEntry { name: "slack", run: run_fetch_cycle },
    NativeEntry { name: "gmail", run: run_fetch_cycle },
    NativeEntry { name: "calendar", run: run_fetch_cycle },
    NativeEntry { name: "vault-maintenance", run: run_vault_maintenance },
    NativeEntry { name: "trello", run: run_trello_sync },
];

// The fetch cycle runs all native fetchers together as one unit.
// Individual fetcher names (gitlab, linear, etc.) all map to the same function
// but we track them as a group under "fetch-cycle".
const FETCH_GROUP: &str = "fetch-cycle";

fn run_fetch_cycle(state: AppState) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let completed = {
            let ps = state.process.lock().await;
            ps.completed.clone()
        };
        let event_tx = {
            let conn = state.connection.lock().await;
            conn.event_tx.clone()
        };
        let new_completed = crate::fetch_cycle::run(completed, event_tx).await;
        let mut ps = state.process.lock().await;
        ps.completed = new_completed;
    })
}

fn run_vault_maintenance(_state: AppState) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        tokio::task::spawn_blocking(|| {
            crate::fetchers::vault_manager::run();
            log("vault-manager", "maintenance complete");
        }).await.ok();
    })
}

fn run_trello_sync(_state: AppState) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        if let Err(e) = crate::trello::sync().await {
            log("trello", &format!("sync failed: {e}"));
        }
    })
}

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(6));

        loop {
            ticker.tick().await;
            run_native_schedules(&state).await;
            run_agentic_schedules(&state).await;
        }
    });
}

async fn run_native_schedules(state: &AppState) {
    let now = Local::now();
    let config = &config::get().schedules;

    // Fetch cycle: gitlab/linear/slack/gmail/calendar run as one unit
    let fetch_names: &[&str] = &["gitlab", "linear", "slack", "gmail", "calendar"];
    let fetch_enabled = fetch_names.iter().any(|n| config.get(*n).is_some_and(|e| e.enabled));
    let fetch_interval = fetch_names.iter()
        .filter_map(|n| config.get(*n).map(|e| e.interval_minutes))
        .min().unwrap_or(60);

    if fetch_enabled {
        let should_run = {
            let mut s = state.scheduler.lock().await;
            if s.running_native.contains(FETCH_GROUP) { false }
            else if s.last_run.get(FETCH_GROUP).is_none_or(|lr| (now - *lr).num_minutes() >= fetch_interval as i64) {
                s.last_run.insert(FETCH_GROUP.to_string(), now);
                s.running_native.insert(FETCH_GROUP.to_string());
                true
            } else { false }
        };
        if should_run {
            log("native-fetchers", "starting fetch cycle");
            let st = state.clone();
            tokio::spawn(async move {
                run_fetch_cycle(st.clone()).await;
                st.scheduler.lock().await.running_native.remove(FETCH_GROUP);
            });
        }
    }

    // Individual native schedules (vault-maintenance)
    for entry in REGISTRY.iter().filter(|e| !fetch_names.contains(&e.name)) {
        let Some(cfg) = config.get(entry.name) else { continue };
        if !cfg.enabled { continue; }

        let should_run = {
            let mut s = state.scheduler.lock().await;
            if s.running_native.contains(entry.name) { false }
            else if s.last_run.get(entry.name).is_none_or(|lr| (now - *lr).num_minutes() >= cfg.interval_minutes as i64) {
                s.last_run.insert(entry.name.to_string(), now);
                s.running_native.insert(entry.name.to_string());
                true
            } else { false }
        };
        if should_run {
            log(entry.name, "starting");
            let st = state.clone();
            let name = entry.name.to_string();
            let fut = (entry.run)(st.clone());
            tokio::spawn(async move {
                fut.await;
                st.scheduler.lock().await.running_native.remove(&name);
            });
        }
    }
}

async fn run_agentic_schedules(state: &AppState) {
    let now = Local::now();

    cleanup_finished(state).await;
    let agents = &config::get().agents;

    // Collect spawn decisions under locks, then execute outside
    struct SpawnDecision {
        index: usize,
        name: String,
        prompt: String,
        cwd: Option<String>,
    }

    let mut to_spawn: Vec<SpawnDecision> = Vec::new();

    {
        let mut sched_state = state.scheduler.lock().await;
        let mut ps = state.process.lock().await;

        for (i, agent) in agents.iter().enumerate() {
            if ps.running.contains_key(&i) { continue; }

            if !agent.depends_on.is_empty() {
                let all_done = agent.depends_on.iter().all(|dep| ps.completed.contains(dep));
                if all_done {
                    log(&agent.name, &format!("triggered by deps: {:?}", agent.depends_on));
                    sched_state.last_run.insert(format!("agent:{}", agent.name), now);
                    for dep in &agent.depends_on { ps.completed.remove(dep); }
                    to_spawn.push(SpawnDecision {
                        index: i, name: agent.name.clone(), prompt: agent.prompt.clone(),
                        cwd: agent.cwd.clone(),
                    });
                }
                continue;
            }

            let key = format!("agent:{}", agent.name);
            let should_run = if agent.interval_minutes == 0 { false } else {
                sched_state.last_run.get(&key).is_none_or(|lr| (now - *lr).num_minutes() >= agent.interval_minutes as i64)
            };
            if should_run {
                log(&agent.name, &format!("scheduled run: {}", agent.prompt.chars().take(60).collect::<String>()));
                sched_state.last_run.insert(key, now);
                to_spawn.push(SpawnDecision {
                    index: i, name: agent.name.clone(), prompt: agent.prompt.clone(),
                    cwd: agent.cwd.clone(),
                });
            }
        }
    } // locks dropped

    // Execute spawns without holding scheduler lock
    for decision in to_spawn {
        if let Some(handle) = spawn_agent(&decision.name, &decision.prompt, decision.cwd.as_deref()) {
            let mut ps = state.process.lock().await;
            ps.running.insert(decision.index, handle);
            drop(ps);
            let conn = state.connection.lock().await;
            if let Some(tx) = &conn.event_tx {
                let _ = tx.send(Event::AgentStarted { index: decision.index, agent: decision.name });
            }
        }
    }
}
