use pasta_common::data::Schedule;
use pasta_common::ipc::{self, Command, Event};
use pasta_common::vault;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::commands;
use crate::state::AppState;
use crate::util::log;

pub async fn run(listener: UnixListener, state: AppState) -> ! {
    loop {
        let (stream, _) = listener.accept().await.expect("accept failed");
        log("backend", "client connected");
        handle_client(stream, state.clone()).await;
        log("backend", "client disconnected");
    }
}

async fn handle_client(stream: UnixStream, state: AppState) {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();

    // Register event sender
    { state.connection.lock().await.event_tx = Some(tx.clone()); }

    // Send initial state
    let snapshot = build_state_snapshot(&state).await;
    if send_event(&mut writer, &snapshot).await.is_err() { return; }

    // Writer task: forward events to client
    let write_handle = tokio::spawn(async move {
        while let Some(evt) = rx.recv().await {
            if send_event(&mut writer, &evt).await.is_err() { break; }
        }
    });

    // Read commands from client
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                if let Ok(cmd) = serde_json::from_str::<Command>(&line) {
                    commands::handle(cmd, &state, &tx).await;
                }
            }
            Err(_) => break,
        }
    }

    // Cleanup
    state.connection.lock().await.event_tx = None;
    write_handle.abort();
}

pub async fn build_state_snapshot(state: &AppState) -> Event {
    let ps = state.process.lock().await;
    let running_indices: Vec<usize> = ps.running.keys().copied().collect();
    drop(ps);

    let sched = state.scheduler.lock().await;
    let config = &pasta_common::config::get().schedules;
    const FETCH_GROUP_NAMES: &[&str] = &["gitlab", "linear", "slack", "gmail", "calendar"];
    let native_schedules: Vec<ipc::NativeSchedule> = pasta_common::data::KNOWN_NATIVE_SCHEDULES.iter().map(|&name| {
        let interval = config.get(name).map_or(60, |e| e.interval_minutes);
        let last_run = sched.last_run.get(name)
            .or_else(|| if FETCH_GROUP_NAMES.contains(&name) { sched.last_run.get("fetch-cycle") } else { None })
            .copied();
        let running = sched.running_native.contains(name) || (FETCH_GROUP_NAMES.contains(&name) && sched.running_native.contains("fetch-cycle"));
        ipc::NativeSchedule { name: name.to_string(), interval_minutes: interval, last_run, running }
    }).collect();
    let agents_config = &pasta_common::config::get().agents;
    let schedules: Vec<Schedule> = agents_config.iter().map(|a| {
        let key = format!("agent:{}", a.name);
        Schedule {
            agent: a.name.clone(),
            prompt: a.prompt.clone(),
            interval_minutes: a.interval_minutes,
            cwd: a.cwd.clone(),
            last_run: sched.last_run.get(&key).copied(),
            depends_on: a.depends_on.clone(),
        }
    }).collect();
    drop(sched);

    Event::State {
        schedules,
        native_schedules,
        running: running_indices,
        tasks: vault::load_tasks(),
        feeds: vault::load_feeds(),
        people: vault::load_people(),
    }
}

async fn send_event(writer: &mut BufWriter<tokio::net::unix::OwnedWriteHalf>, evt: &Event) -> Result<(), std::io::Error> {
    let mut s = serde_json::to_string(evt).map_err(std::io::Error::other)?;
    s.push('\n');
    writer.write_all(s.as_bytes()).await?;
    writer.flush().await
}
