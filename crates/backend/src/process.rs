use chrono::Local;
use std::fs;
use std::process::Stdio;
use tokio::process;

use pasta_common::config;
use pasta_common::ipc::Event;

use crate::state::AppState;
use crate::util::{log, log_dir};

pub async fn cleanup_finished(state: &AppState) {
    let agents = &config::get().agents;
    let mut ps = state.process.lock().await;
    let mut finished = Vec::new();

    ps.running.retain(|idx, handle| {
        if handle.is_finished() {
            if let Some(s) = agents.get(*idx) {
                finished.push((*idx, s.name.clone()));
            }
            false
        } else {
            true
        }
    });

    if !finished.is_empty() {
        for (_idx, agent) in &finished {
            ps.completed.insert(agent.clone());
        }
        drop(ps);
        let conn = state.connection.lock().await;
        for (idx, agent) in &finished {
            if let Some(tx) = &conn.event_tx {
                let _ = tx.send(Event::AgentFinished { index: *idx, agent: agent.clone() });
            }
        }
    }
}

pub fn spawn_agent(agent: &str, prompt: &str, cwd: Option<&str>) -> Option<tokio::task::JoinHandle<()>> {
    let ts = Local::now().format("%Y%m%d-%H%M%S");
    let log_file = log_dir().join(format!("{}_{}.log", agent, ts));

    let stdout_file = fs::File::create(&log_file).unwrap_or_else(|_| fs::File::create("/dev/null").unwrap());
    let stderr_file = stdout_file.try_clone().unwrap_or_else(|_| fs::File::create("/dev/null").unwrap());

    let kiro = crate::fetchers::resolve_binary("kiro-cli", "KIRO_CLI_PATH", "/home/orre/.nix-profile/bin/kiro-cli");
    let mut cmd = process::Command::new(&kiro);
    cmd.args(["chat", "--no-interactive", "--trust-all-tools", "--agent", agent, prompt])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    if let Some(dir) = cwd { cmd.current_dir(dir); }

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id().unwrap_or(0);
            let agent_name = agent.to_string();
            log(&agent_name, &format!("spawned pid={} → {}", pid, log_file.display()));
            let timeout_mins = config::get().general.agent_timeout_minutes;
            Some(tokio::spawn(async move {
                let timeout_dur = tokio::time::Duration::from_secs(timeout_mins * 60);
                match tokio::time::timeout(timeout_dur, child.wait()).await {
                    Ok(_) => {}
                    Err(_) => {
                        child.kill().await.ok();
                        log(&agent_name, "TIMEOUT — killed");
                    }
                }
            }))
        }
        Err(e) => { log(agent, &format!("FAILED to spawn: {}", e)); None }
    }
}
