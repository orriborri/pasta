use pasta_common::ipc::{ChatEventType, Command, Event};

use crate::{acp, fetch_cycle, server};
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
                let fetch_names = ["gitlab", "linear", "slack", "gmail", "calendar"];
                if fetch_names.contains(&name.as_str()) {
                    sched.last_run.remove("fetch-cycle");
                }
                drop(sched);
                let _ = tx.send(Event::Flash { message: format!("↻ {} triggered", name) });
            }
        }
        Command::ForceFetch => {
            log("native-fetchers", "force fetch triggered");
            let st = state.clone();
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
                let mut ps = st.process.lock().await;
                ps.completed = new_completed;
            });
        }
        Command::Backfill => {
            let tx2 = tx.clone();
            log("kb-sync", "manual sync triggered");
            tokio::spawn(async move {
                let _ = tokio::process::Command::new("kb").arg("sync").status().await;
                let _ = tx2.send(Event::Flash { message: "kb sync complete".to_string() });
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
        Command::ChatStart { agent } => {
            let mut conn = state.connection.lock().await;
            if let Some(mut handle) = conn.acp.take() {
                let _ = handle.child.start_kill();
            }
            match acp::spawn(&agent, tx.clone()).await {
                Ok(handle) => { conn.acp = Some(handle); }
                Err(e) => {
                    let _ = tx.send(Event::Chat { chat_type: ChatEventType::Error { message: e.to_string() } });
                }
            }
        }
        Command::ChatPrompt { text } => {
            let mut conn = state.connection.lock().await;
            if let Some(handle) = conn.acp.as_mut() {
                let _ = acp::send_prompt(handle, &text).await;
            }
        }
        Command::ChatStop => {
            let mut conn = state.connection.lock().await;
            if let Some(mut handle) = conn.acp.take() {
                let _ = handle.child.start_kill();
            }
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
