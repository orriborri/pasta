use pasta_common::ipc::{Command, Event};

use crate::{fetch_cycle, server};
use crate::process::spawn_agent;
use crate::state::{AppState, EventTx};
use crate::util::log;

pub async fn handle(cmd: Command, state: &AppState, tx: &EventTx) {
    match cmd {
        Command::Sync => {
            let snapshot = server::build_state_snapshot(state).await;
            let _ = tx.send(snapshot);
        }
        Command::RunNow { index } => {
            let agents = &pasta_common::config::get().agents;
            if let Some(s) = agents.get(index) {
                let ps = state.process.lock().await;
                if ps.running.contains_key(&index) {
                    let _ = tx.send(Event::Flash { message: format!("{} already running", s.name) });
                    return;
                }
                drop(ps);
                let agent = s.name.clone();
                let prompt = s.prompt.clone();
                let cwd = s.cwd.clone();
                log(&agent, &format!("manual run: {}", prompt.chars().take(60).collect::<String>()));
                if let Some(handle) = spawn_agent(&agent, &prompt, cwd.as_deref()) {
                    state.process.lock().await.running.insert(index, handle);
                    let _ = tx.send(Event::AgentStarted { index, agent: agent.clone() });
                    let _ = tx.send(Event::Flash { message: format!("↻ {} triggered", agent) });
                }
            }
        }
        Command::RerunNative { name } => {
            let mut sched = state.scheduler.lock().await;
            if sched.running_native.contains(&name) {
                let _ = tx.send(Event::Flash { message: format!("{} already running", name) });
            } else {
                // Reset timer so next tick fires it
                sched.last_run.remove(&name);
                // For fetch-group members, also reset the group
                if pasta_common::data::FETCH_GROUP_NAMES.contains(&name.as_str()) {
                    sched.last_run.remove("fetch-cycle");
                }
                drop(sched);
                let _ = tx.send(Event::Flash { message: format!("↻ {} triggered", name) });
            }
        }
        Command::ForceFetch => {
            let st = state.clone();
            let tx_clone = tx.clone();
            let should_run = {
                let mut sched = st.scheduler.lock().await;
                if sched.running_native.contains("fetch-cycle") {
                    let _ = tx_clone.send(Event::Flash { message: "fetch-cycle already running".to_string() });
                    false
                } else {
                    sched.running_native.insert("fetch-cycle".to_string());
                    true
                }
            };
            if !should_run {
                return;
            }
            log("native-fetchers", "force fetch triggered");
            tokio::spawn(async move {
                let completed = {
                    let ps = st.process.lock().await;
                    ps.completed.clone()
                };
                let event_tx = {
                    let conn = st.connection.lock().await;
                    conn.event_tx.clone()
                };
                let new_completed = fetch_cycle::run(completed, event_tx).await;
                {
                    let mut ps = st.process.lock().await;
                    ps.completed = new_completed;
                }
                {
                    let mut sched = st.scheduler.lock().await;
                    sched.running_native.remove("fetch-cycle");
                }
            });
        }
        Command::Backfill => {
            let tx2 = tx.clone();
            log("kb-sync", "manual sync triggered");
            tokio::spawn(async move {
                let sources = &["slack", "gmail", "linear", "git", "vault", "calendar", "gdocs"];
                match kb_sync::run(sources).await {
                    Ok(n) => {
                        let _ = tx2.send(Event::Flash { message: format!("kb sync complete: {n} records") });
                    }
                    Err(e) => {
                        let _ = tx2.send(Event::Flash { message: format!("kb sync failed: {e}") });
                    }
                }
            });
        }
        Command::Organize => {
            let tx2 = tx.clone();
            log("vault-organize", "manual organize triggered");
            tokio::spawn(async move {
                let _ = crate::vault_organize::repair_links().await;
                let _ = crate::vault_organize::audit_para().await;
                let _ = tx2.send(Event::Flash { message: "Vault organize complete".to_string() });
            });
        }
        Command::Search { query, source, participant, after, limit } => {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let lim = limit.unwrap_or(10);
                match crate::kb_search::search(
                    &query,
                    source.as_deref(),
                    participant.as_deref(),
                    after.as_deref(),
                    lim,
                ).await {
                    Ok(results) => {
                        let _ = tx2.send(Event::SearchResults { results });
                    }
                    Err(e) => {
                        let _ = tx2.send(Event::Flash { message: format!("Search error: {}", e) });
                    }
                }
            });
        }
    }
}
