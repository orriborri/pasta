mod ui;

use pasta_common::ipc::{self, ChatEventType, Command, Event};
use pasta_common::vault;
use ui::{App, ChatMessage, ChatRole, InputField, LayoutCache, Mode, Tab, UndoAction};

use crossterm::{
    event::{self, Event as CEvent, KeyCode, MouseEvent, MouseEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use std::io::stdout;
use std::process::Command as StdCommand;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sock_path = ipc::socket_path();
    let stream = connect_or_start_backend(&sock_path).await?;

    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let writer = tokio::sync::Mutex::new(writer);

    // Channel for events from backend
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<Event>();

    // Reader task
    tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(evt) = serde_json::from_str::<Event>(&line) {
                        let _ = evt_tx.send(evt);
                    }
                }
                Err(_) => break,
            }
        }
    });

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(crossterm::event::EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new();
    let mut tick_counter = 0u32;

    // Defer good-morning until fetchers complete
    let mut pending_good_morning = !ui::today_logged();
    if pending_good_morning {
        app.sync_progress = Some("Waiting for fetchers...".to_string());
    }

    // Periodically pull a fresh state snapshot so schedule last_run / next-run
    // status stays accurate. Without this the TUI shows stale "overdue" rows
    // because last_run is only updated when an Event::State arrives.
    let mut last_sync = std::time::Instant::now();

    while app.running {
        app.tick = tick_counter;

        // Expire flash/undo
        if app.flash.as_ref().is_some_and(|f| f.at.elapsed().as_secs() >= 3) { app.flash = None; }
        if app.undo.as_ref().is_some_and(|u| u.at.elapsed().as_secs() >= 5) { app.undo = None; }

        // Drain backend events
        while let Ok(evt) = evt_rx.try_recv() {
            apply_event(&mut app, evt);
        }

        // Refresh state from the backend every 2s
        if last_sync.elapsed() >= Duration::from_secs(2) {
            send_cmd(&writer, &Command::Sync).await;
            last_sync = std::time::Instant::now();
        }

        // Start good-morning after fetchers complete
        if pending_good_morning && app.fetch_completed {
            pending_good_morning = false;
            app.mode = Mode::Chat;
            app.chat_messages.clear();
            app.chat_messages.push(ChatMessage { role: ChatRole::System, text: "Starting good-morning agent...".to_string() });
            app.chat_streaming = true;
            app.chat_agent = "good-morning".to_string();
            send_cmd(&writer, &Command::ChatStart { agent: "good-morning".to_string() }).await;
            send_cmd(&writer, &Command::ChatPrompt { text: "Good morning".to_string() }).await;
        }

        let mut layout = LayoutCache::default();
        terminal.draw(|f| { layout = ui::draw(f, &app); })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                CEvent::Key(key) => match &app.mode {
                    Mode::Normal => handle_normal_key(&mut app, key.code, &writer, &mut terminal).await,
                    Mode::Adding { .. } => handle_input_key(&mut app, key.code, &writer).await,
                    Mode::QuickCapture => handle_capture_key(&mut app, key.code),
                    Mode::Filter => handle_filter_key(&mut app, key.code, &writer).await,
                    Mode::Chat => handle_chat_key(&mut app, key.code, &writer).await,
                },
                CEvent::Mouse(mouse) => handle_mouse(&mut app, mouse, &layout),
                _ => {}
            }
        }

        tick_counter = (tick_counter + 1) % 60;
    }

    disable_raw_mode()?;
    stdout().execute(crossterm::event::DisableMouseCapture)?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

async fn connect_or_start_backend(sock_path: &std::path::Path) -> anyhow::Result<UnixStream> {
    // Try connecting to existing socket
    if sock_path.exists() {
        if let Ok(stream) = UnixStream::connect(sock_path).await {
            return Ok(stream);
        }
        // Socket exists but connection failed — check for running backends
        let pids = find_backend_pids();
        if !pids.is_empty() {
            eprintln!("Found running pasta-backend process(es): {:?}", pids);
            eprint!("Kill them and start fresh? [Y/n] ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            let input = input.trim().to_lowercase();
            if input.is_empty() || input == "y" || input == "yes" {
                for pid in &pids {
                    StdCommand::new("kill").arg(pid.to_string()).output().ok();
                }
                std::thread::sleep(Duration::from_millis(500));
            } else {
                eprintln!("Aborting.");
                std::process::exit(1);
            }
        }
        // Remove stale socket
        std::fs::remove_file(sock_path).ok();
    }

    // Start backend
    start_backend()?;
    for _ in 0..30 {
        if sock_path.exists() {
            if let Ok(stream) = UnixStream::connect(sock_path).await {
                return Ok(stream);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    anyhow::bail!("Failed to connect to backend after starting it")
}

fn find_backend_pids() -> Vec<u32> {
    let output = StdCommand::new("pgrep").arg("-f").arg("pasta-backend").output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect(),
        Err(_) => vec![],
    }
}

fn start_backend() -> anyhow::Result<()> {
    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let backend_path = exe_dir.join("pasta-backend");
    // The pasta project root is 3 levels up from target/release/pasta-backend
    let pasta_dir = exe_dir.parent().unwrap().parent().unwrap();
    let log_dir = pasta_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    let log_file = std::fs::File::create(log_dir.join("daemon.log"))?;
    let log_err = log_file.try_clone()?;
    StdCommand::new(backend_path)
        .current_dir(pasta_dir)
        .stdin(std::process::Stdio::null())
        .stdout(log_file)
        .stderr(log_err)
        .spawn()?;
    Ok(())
}

fn apply_event(app: &mut App, evt: Event) {
    match evt {
        Event::State { schedules, native_schedules, running, tasks, feeds, people } => {
            // Derive fetch completion from the snapshot: if a native fetcher has a
            // last_run dated today, the fetchers have run this session even if we
            // missed the transient FetchComplete event (e.g. it fired before the
            // TUI connected). This lets good-morning proceed instead of waiting
            // forever for an event that already happened.
            const FETCH_GROUP: &[&str] = &["gitlab", "linear", "slack", "gmail", "calendar"];
            let today = chrono::Local::now().date_naive();
            if native_schedules.iter().any(|ns| {
                FETCH_GROUP.contains(&ns.name.as_str())
                    && ns.last_run.is_some_and(|lr| lr.date_naive() == today)
            }) {
                app.fetch_completed = true;
            }
            app.schedules = schedules;
            app.native_schedules = native_schedules;
            app.running_indices = running;
            app.tasks = tasks;
            app.feeds = feeds;
            app.people = people;
        }
        Event::AgentStarted { index, .. } => {
            if !app.running_indices.contains(&index) {
                app.running_indices.push(index);
            }
        }
        Event::AgentFinished { index, .. } => {
            app.running_indices.retain(|&i| i != index);
        }
        Event::FetchComplete => {
            app.feeds = vault::load_feeds();
            app.daily_note = ui::load_daily_note();
            app.fetch_completed = true;
            app.flash("Fetch complete".to_string());
        }
        Event::Chat { chat_type } => {
            match chat_type {
                ChatEventType::Text { text } => {
                    if let Some(last) = app.chat_messages.last_mut() {
                        if last.role == ChatRole::Agent {
                            last.text.push_str(&text);
                        } else {
                            app.chat_messages.push(ChatMessage { role: ChatRole::Agent, text });
                        }
                    } else {
                        app.chat_messages.push(ChatMessage { role: ChatRole::Agent, text });
                    }
                }
                ChatEventType::ToolCall { name, status } => {
                    app.chat_messages.push(ChatMessage {
                        role: ChatRole::System,
                        text: format!("[{}] {}", status, name),
                    });
                }
                ChatEventType::TurnEnd => { app.chat_streaming = false; }
                ChatEventType::Error { message } => {
                    app.chat_messages.push(ChatMessage {
                        role: ChatRole::System,
                        text: format!("Error: {}", message),
                    });
                    app.chat_streaming = false;
                }
            }
        }
        Event::Flash { message } => { app.flash(message); }
        Event::SearchResults { results } => { app.search_results = results; app.selected = 0; }
        Event::SyncProgress { stage, done, total } => {
            if stage == "done" {
                app.sync_progress = None;
            } else {
                app.sync_progress = Some(format!("sync: {} {}/{}", stage, done, total));
            }
        }
    }
}

async fn send_cmd(writer: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>, cmd: &Command) {
    let mut s = serde_json::to_string(cmd).unwrap();
    s.push('\n');
    let mut w = writer.lock().await;
    let _ = w.write_all(s.as_bytes()).await;
    let _ = w.flush().await;
}

async fn handle_normal_key(
    app: &mut App,
    key: KeyCode,
    writer: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match key {
        KeyCode::Char('q') => app.running = false,
        KeyCode::Char('1') => { app.tab = Tab::Today; app.selected = 0; }
        KeyCode::Char('2') => { app.tab = Tab::Tasks; app.selected = 0; }
        KeyCode::Char('3') => { app.tab = Tab::Search; app.selected = 0; }
        KeyCode::Char('4') => { app.tab = Tab::People; app.selected = 0; }
        KeyCode::Char('5') => { app.tab = Tab::Feeds; app.selected = 0; }
        KeyCode::Char('6') => { app.tab = Tab::Agents; app.selected = 0; }
        KeyCode::Char('7') => { app.tab = Tab::Schedule; app.selected = 0; }
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),
        KeyCode::Char('r') if app.tab == Tab::Schedule => {
            let native_count = app.native_schedules.len();
            if app.selected < native_count {
                let name = app.native_schedules[app.selected].name.clone();
                send_cmd(writer, &Command::RerunNative { name }).await;
            } else {
                let index = app.selected - native_count;
                if index < app.schedules.len() {
                    send_cmd(writer, &Command::RunNow { index }).await;
                }
            }
        }
        KeyCode::Char('r') => {
            send_cmd(writer, &Command::Sync).await;
            app.daily_note = ui::load_daily_note();
        }
        KeyCode::Char('w') => {
            app.mode = Mode::Chat;
            app.input_buf.clear();
            app.chat_messages.clear();
            app.chat_streaming = false;
            app.chat_agent = "personal-assistant".to_string();
            send_cmd(writer, &Command::ChatStart { agent: "personal-assistant".to_string() }).await;
        }
        KeyCode::Char('g') => {
            app.mode = Mode::Chat;
            app.input_buf.clear();
            app.chat_messages.clear();
            app.chat_streaming = false;
            app.chat_agent = "end-of-day".to_string();
            send_cmd(writer, &Command::ChatStart { agent: "end-of-day".to_string() }).await;
        }
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Char('/') if app.tab == Tab::Tasks || app.tab == Tab::Search => {
            app.mode = Mode::Filter;
            app.input_buf = app.filter.clone();
        }
        KeyCode::Char('c') if app.tab == Tab::Tasks => app.start_capture(),
        KeyCode::Enter if app.tab == Tab::Today => {
            app.mode = Mode::Chat;
            app.input_buf.clear();
            app.chat_messages.clear();
            app.chat_streaming = false;
            app.chat_agent = "personal-assistant".to_string();
            send_cmd(writer, &Command::ChatStart { agent: "personal-assistant".to_string() }).await;
        }
        KeyCode::Char('x') if app.tab == Tab::Today => {
            toggle_daily_task(app, app.selected);
        }
        KeyCode::Char('e') | KeyCode::Enter if app.tab == Tab::Tasks => {
            if let Some(task) = app.selected_task() {
                let file = task.file.clone();
                open_in_editor(&file, terminal);
                app.tasks = vault::load_tasks();
            }
        }
        KeyCode::Char('x') if app.tab == Tab::Tasks => {
            if let Some(task) = app.selected_task() {
                let task = task.clone();
                let title = task.title.clone();
                app.undo = Some(UndoAction { task: task.clone(), at: std::time::Instant::now() });
                vault::close_task(&task);
                app.tasks = vault::load_tasks();
                if app.selected >= app.filtered_tasks().len() {
                    app.selected = app.filtered_tasks().len().saturating_sub(1);
                }
                app.flash(format!("Closed: {}", title));
            }
        }
        KeyCode::Char('a') if app.tab == Tab::Tasks => {
            match app.selected_task().cloned() {
                Some(task) if task.is_pending() => {
                    let title = task.title.clone();
                    app.undo = Some(UndoAction { task: task.clone(), at: std::time::Instant::now() });
                    vault::approve_task(&task);
                    app.tasks = vault::load_tasks();
                    app.flash(format!("Approved: {}", title));
                }
                Some(_) => app.flash("Not pending approval".to_string()),
                None => {}
            }
        }
        KeyCode::Char('d') if app.tab == Tab::Tasks => {
            match app.selected_task().cloned() {
                Some(task) if task.is_pending() => {
                    let title = task.title.clone();
                    app.undo = Some(UndoAction { task: task.clone(), at: std::time::Instant::now() });
                    vault::reject_task(&task);
                    app.tasks = vault::load_tasks();
                    if app.selected >= app.filtered_tasks().len() {
                        app.selected = app.filtered_tasks().len().saturating_sub(1);
                    }
                    app.flash(format!("Rejected: {}", title));
                }
                Some(_) => app.flash("Only pending tasks can be rejected".to_string()),
                None => {}
            }
        }
        KeyCode::Char('u') if app.tab == Tab::Tasks => {
            if let Some(undo) = app.undo.take() {
                if undo.at.elapsed().as_secs() < 5 {
                    vault::reopen_task(&undo.task);
                    app.tasks = vault::load_tasks();
                    app.flash(format!("Undo: status → {}", undo.task.status));
                }
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') if app.tab == Tab::Tasks => {
            if let Some(task) = app.selected_task() {
                let task = task.clone();
                let cur: u8 = task.priority.parse().unwrap_or(4);
                let new_pri = cur.saturating_sub(1).max(1);
                vault::update_task_priority(&task, &new_pri.to_string());
                app.tasks = vault::load_tasks();
                app.flash(format!("Priority → {}", new_pri));
            }
        }
        KeyCode::Char('-') if app.tab == Tab::Tasks => {
            if let Some(task) = app.selected_task() {
                let task = task.clone();
                let cur: u8 = task.priority.parse().unwrap_or(0);
                let new_pri = (cur + 1).min(4);
                vault::update_task_priority(&task, &new_pri.to_string());
                app.tasks = vault::load_tasks();
                app.flash(format!("Priority → {}", new_pri));
            }
        }
        // Schedule tab
        KeyCode::Char('a') if app.tab == Tab::Schedule => app.flash("Edit ~/.pasta/config.toml to add agents".to_string()),
        KeyCode::Char('f') if app.tab == Tab::Schedule => {
            send_cmd(writer, &Command::ForceFetch).await;
            app.flash("Fetchers triggered".to_string());
        }
        KeyCode::Char('b') if app.tab == Tab::Schedule => {
            send_cmd(writer, &Command::Backfill).await;
            app.flash("Backfill started...".to_string());
        }
        KeyCode::Char('o') if app.tab == Tab::Schedule => {
            send_cmd(writer, &Command::Organize).await;
            app.flash("Vault organize started...".to_string());
        }
        KeyCode::Char('d') if app.tab == Tab::Schedule => {
            app.flash("Edit ~/.pasta/config.toml to remove agents".to_string());
        }
        KeyCode::Enter if app.tab == Tab::Schedule => {
            let native_count = app.native_schedules.len();
            if app.selected < native_count {
                let name = app.native_schedules[app.selected].name.clone();
                send_cmd(writer, &Command::RerunNative { name }).await;
            } else {
                let index = app.selected - native_count;
                if index < app.schedules.len() {
                    send_cmd(writer, &Command::RunNow { index }).await;
                }
            }
        }
        KeyCode::Enter if app.tab == Tab::Agents
            && app.selected < app.agents.len() => {
                let agent = app.agents[app.selected].name.clone();
                app.mode = Mode::Chat;
                app.input_buf.clear();
                app.chat_messages.clear();
                app.chat_streaming = false;
                app.chat_agent = agent.clone();
                send_cmd(writer, &Command::ChatStart { agent }).await;
            }
        _ => {}
    }
}

fn toggle_daily_task(app: &mut App, task_idx: usize) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let path = std::path::Path::new(vault::vault_path()).join(format!("0. Inbox/Daily/{}.md", today));
    let mut lines: Vec<String> = app.daily_note.lines().map(|l| l.to_string()).collect();
    // Find the nth task line
    let mut count = 0;
    for line in lines.iter_mut() {
        if line.starts_with("- [ ]") || line.starts_with("- [x]") {
            if count == task_idx {
                if line.starts_with("- [ ]") {
                    *line = line.replacen("- [ ]", "- [x]", 1);
                } else {
                    *line = line.replacen("- [x]", "- [ ]", 1);
                }
                break;
            }
            count += 1;
        }
    }
    let content = lines.join("\n");
    std::fs::write(&path, &content).ok();
    app.daily_note = content;
}

fn open_in_editor(file: &str, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let path = std::path::Path::new(vault::vault_path()).join("Tasks").join(file);
    disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();
    StdCommand::new(&editor).arg(&path).status().ok();
    stdout().execute(EnterAlternateScreen).ok();
    enable_raw_mode().ok();
    terminal.clear().ok();
}

async fn handle_filter_key(app: &mut App, key: KeyCode, writer: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>) {
    match key {
        KeyCode::Esc => { app.mode = Mode::Normal; }
        KeyCode::Enter => {
            app.filter = app.input_buf.clone();
            app.selected = 0;
            app.mode = Mode::Normal;
            if app.tab == Tab::Search && !app.filter.is_empty() {
                send_cmd(writer, &Command::Search {
                    query: app.filter.clone(),
                    source: None, participant: None, after: None, limit: Some(10),
                }).await;
            }
        }
        KeyCode::Backspace => { app.input_buf.pop(); app.filter = app.input_buf.clone(); app.selected = 0; }
        KeyCode::Char(c) => { app.input_buf.push(c); app.filter = app.input_buf.clone(); app.selected = 0; }
        _ => {}
    }
}

async fn handle_input_key(app: &mut App, key: KeyCode, writer: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>) {
    match key {
        KeyCode::Esc => { app.mode = Mode::Normal; app.input_buf.clear(); }
        KeyCode::Backspace => { app.input_buf.pop(); }
        KeyCode::Char(c) => app.input_buf.push(c),
        KeyCode::Enter => advance_input(app, writer).await,
        _ => {}
    }
}

fn handle_capture_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => { app.mode = Mode::Normal; app.input_buf.clear(); }
        KeyCode::Backspace => { app.input_buf.pop(); }
        KeyCode::Char(c) => app.input_buf.push(c),
        KeyCode::Enter => {
            if !app.input_buf.is_empty() {
                vault::add_to_inbox(&app.input_buf);
                app.flash("Added to inbox".to_string());
            }
            app.input_buf.clear();
            app.mode = Mode::Normal;
        }
        _ => {}
    }
}

async fn advance_input(app: &mut App, _writer: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>) {
    let buf = app.input_buf.clone();
    let mode = std::mem::replace(&mut app.mode, Mode::Normal);

    if let Mode::Adding { field, mut agent, interval: _, prompt: _ } = mode {
        match field {
            InputField::Agent => {
                agent = buf;
                app.input_buf.clear();
                app.mode = Mode::Adding { field: InputField::Interval, agent, interval: String::new(), prompt: String::new() };
            }
            InputField::Interval => {
                let interval = buf;
                app.input_buf.clear();
                app.mode = Mode::Adding { field: InputField::Prompt, agent, interval, prompt: String::new() };
            }
            InputField::Prompt => {
                app.input_buf.clear();
                app.flash("Edit ~/.pasta/config.toml to add agents".to_string());
            }
        }
    }
}

async fn handle_chat_key(app: &mut App, key: KeyCode, writer: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>) {
    match key {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.input_buf.clear();
            send_cmd(writer, &Command::ChatStop).await;
        }
        KeyCode::Backspace => { app.input_buf.pop(); }
        KeyCode::Char(c) => app.input_buf.push(c),
        KeyCode::Enter => {
            if app.input_buf.is_empty() { return; }
            let text = app.input_buf.clone();
            app.input_buf.clear();
            app.chat_messages.push(ChatMessage { role: ChatRole::User, text: text.clone() });
            app.chat_streaming = true;
            send_cmd(writer, &Command::ChatPrompt { text }).await;
        }
        _ => {}
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent, layout: &LayoutCache) {
    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            let col = mouse.column;
            let row = mouse.row;

            // Tab click
            if app.mode == Mode::Normal && layout.tabs_rect.contains((col, row).into()) {
                let tab_width = layout.tabs_rect.width / 7;
                let idx = ((col - layout.tabs_rect.x) / tab_width).min(6) as usize;
                let tabs = [Tab::Today, Tab::Tasks, Tab::Search, Tab::People, Tab::Feeds, Tab::Agents, Tab::Schedule];
                app.tab = tabs[idx];
                app.selected = 0;
                return;
            }

            // List item click
            if app.mode == Mode::Normal && layout.content_rect.contains((col, row).into()) {
                // Account for border (1 row) + header (1 row for tables)
                let offset = row.saturating_sub(layout.content_rect.y + 2) as usize;
                let max = list_len(app);
                if offset < max {
                    app.selected = offset;
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if app.mode == Mode::Chat {
                app.chat_scroll_offset = app.chat_scroll_offset.saturating_add(3);
            } else {
                app.select_next();
            }
        }
        MouseEventKind::ScrollUp => {
            if app.mode == Mode::Chat {
                app.chat_scroll_offset = app.chat_scroll_offset.saturating_sub(3);
            } else {
                app.select_prev();
            }
        }
        _ => {}
    }
}

fn list_len(app: &App) -> usize {
    match app.tab {
        Tab::Today => app.daily_task_count(),
        Tab::Tasks => app.filtered_tasks().len(),
        Tab::Schedule => app.native_schedules.len() + app.schedules.len(),
        Tab::People => app.people.len(),
        Tab::Agents => app.agents.len(),
        Tab::Search => app.search_results.len(),
        _ => 0,
    }
}
