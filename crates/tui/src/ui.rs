use pasta_common::data::Schedule;
use pasta_common::ipc;
use pasta_common::vault::{self, Feed, Person, Task};
use chrono::{Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Frame,
};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutCache {
    pub tabs_rect: Rect,
    pub content_rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab { Today, Tasks, Search, People, Feeds, Agents, Schedule }

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
}

pub fn discover_agents() -> Vec<AgentInfo> {
    let mut agents: HashMap<String, AgentInfo> = HashMap::new();
    // Global agents
    if let Some(home) = dirs::home_dir() {
        scan_agents_dir(&home.join(".kiro/agents"), &mut agents);
    }
    // Vault-local agents (override global)
    scan_agents_dir(std::path::Path::new(vault::vault_path()).join(".kiro/agents").as_path(), &mut agents);
    let mut result: Vec<AgentInfo> = agents.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn scan_agents_dir(dir: &std::path::Path, agents: &mut HashMap<String, AgentInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let name = val.get("name").and_then(|n| n.as_str())
                        .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_str().unwrap_or(""))
                        .to_string();
                    let description = val.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                    if !name.is_empty() {
                        agents.insert(name.clone(), AgentInfo { name, description });
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputField { Agent, Interval, Prompt }

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Adding { field: InputField, agent: String, interval: String, prompt: String },
    QuickCapture,
    Filter,
    Chat,
}

pub struct FlashMessage { pub text: String, pub at: Instant }
pub struct UndoAction { pub task: Task, pub at: Instant }

#[derive(Debug, Clone)]
pub struct ChatMessage { pub role: ChatRole, pub text: String }

#[derive(Debug, Clone, PartialEq)]
pub enum ChatRole { User, Agent, System }

pub struct App {
    pub tab: Tab,
    pub mode: Mode,
    pub schedules: Vec<Schedule>,
    pub native_schedules: Vec<ipc::NativeSchedule>,
    pub tasks: Vec<Task>,
    pub feeds: Vec<Feed>,
    pub people: Vec<Person>,
    pub selected: usize,
    pub running: bool,
    pub input_buf: String,
    pub running_indices: Vec<usize>,
    pub tick: u32,
    pub chat_messages: Vec<ChatMessage>,
    pub chat_streaming: bool,
    pub chat_agent: String,
    pub agents: Vec<AgentInfo>,
    pub flash: Option<FlashMessage>,
    pub undo: Option<UndoAction>,
    pub filter: String,
    pub sync_progress: Option<String>,
    pub fetch_completed: bool,
    pub daily_note: String,
    pub search_results: Vec<ipc::SearchResultItem>,
    pub chat_scroll_offset: u16,
}

impl App {
    pub fn new() -> Self {
        Self {
            tab: Tab::Today,
            mode: Mode::Normal,
            schedules: Vec::new(),
            native_schedules: Vec::new(),
            tasks: Vec::new(),
            feeds: Vec::new(),
            people: Vec::new(),
            selected: 0,
            running: true,
            input_buf: String::new(),
            running_indices: Vec::new(),
            tick: 0,
            chat_messages: Vec::new(),
            chat_streaming: false,
            chat_agent: String::new(),
            agents: discover_agents(),
            flash: None,
            undo: None,
            filter: String::new(),
            sync_progress: None,
            fetch_completed: false,
            daily_note: load_daily_note(),
            search_results: Vec::new(),
            chat_scroll_offset: 0,
        }
    }

    pub fn flash(&mut self, msg: String) {
        self.flash = Some(FlashMessage { text: msg, at: Instant::now() });
    }

    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Today => Tab::Tasks, Tab::Tasks => Tab::Search,
            Tab::Search => Tab::People, Tab::People => Tab::Feeds,
            Tab::Feeds => Tab::Agents, Tab::Agents => Tab::Schedule,
            Tab::Schedule => Tab::Today,
        };
        self.selected = 0; self.filter.clear();
    }

    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Today => Tab::Schedule, Tab::Tasks => Tab::Today,
            Tab::Search => Tab::Tasks, Tab::People => Tab::Search,
            Tab::Feeds => Tab::People, Tab::Agents => Tab::Feeds,
            Tab::Schedule => Tab::Agents,
        };
        self.selected = 0; self.filter.clear();
    }

    pub fn filtered_tasks(&self) -> Vec<&Task> {
        if self.filter.is_empty() {
            self.tasks.iter().collect()
        } else {
            let f = self.filter.to_lowercase();
            self.tasks.iter().filter(|t| t.title.to_lowercase().contains(&f) || t.project.to_lowercase().contains(&f)).collect()
        }
    }

    pub fn daily_task_count(&self) -> usize {
        self.daily_note.lines()
            .filter(|l| l.starts_with("- [ ]") || l.starts_with("- [x]"))
            .count()
    }

    pub fn select_next(&mut self) {
        let max = match self.tab {
            Tab::Today => self.daily_task_count(),
            Tab::Schedule => self.native_schedules.len() + self.schedules.len(),
            Tab::Tasks => self.filtered_tasks().len(),
            Tab::People => self.people.len(),
            Tab::Agents => self.agents.len(),
            Tab::Search => self.search_results.len(),
            _ => 0,
        };
        if max > 0 && self.selected < max - 1 { self.selected += 1; }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 { self.selected -= 1; }
    }

    pub fn start_capture(&mut self) {
        self.mode = Mode::QuickCapture;
        self.input_buf.clear();
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.filtered_tasks().get(self.selected).copied()
    }
}

pub fn draw(f: &mut Frame, app: &App) -> LayoutCache {
    if app.mode == Mode::Chat { draw_chat(f, app); return LayoutCache::default(); }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let layout = LayoutCache { tabs_rect: chunks[0], content_rect: chunks[1] };

    let titles = ["Today [1]", "Tasks [2]", "Search [3]", "People [4]", "Feeds [5]", "Agents [6]", "Schedule [7]"];
    let selected = match app.tab {
        Tab::Today => 0, Tab::Tasks => 1, Tab::Search => 2, Tab::People => 3, Tab::Feeds => 4, Tab::Agents => 5, Tab::Schedule => 6,
    };
    let tabs = Tabs::new(titles.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
        .select(selected)
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title(" pasta "));
    f.render_widget(tabs, chunks[0]);

    match app.tab {
        Tab::Today => draw_today(f, app, chunks[1]),
        Tab::Tasks => draw_tasks(f, app, chunks[1]),
        Tab::Search => draw_search(f, app, chunks[1]),
        Tab::People => draw_people(f, app, chunks[1]),
        Tab::Feeds => draw_feeds(f, app, chunks[1]),
        Tab::Agents => draw_agents(f, app, chunks[1]),
        Tab::Schedule => draw_schedule(f, app, chunks[1]),
    }

    let bar_content = if let Some(flash) = &app.flash {
        if flash.at.elapsed().as_secs() < 3 {
            Line::from(Span::styled(format!(" ✓ {}", flash.text), Style::default().fg(Color::Green)))
        } else if let Some(prog) = &app.sync_progress {
            Line::from(Span::styled(format!(" ⟳ {}", prog), Style::default().fg(Color::Yellow)))
        } else { help_line(app) }
    } else if let Some(prog) = &app.sync_progress {
        Line::from(Span::styled(format!(" ⟳ {}", prog), Style::default().fg(Color::Yellow)))
    } else { help_line(app) };
    f.render_widget(Paragraph::new(bar_content), chunks[2]);

    match &app.mode {
        Mode::Adding { field, agent, interval, prompt } => draw_input_dialog(f, field, agent, interval, prompt, &app.input_buf),
        Mode::QuickCapture => draw_capture_dialog(f, &app.input_buf),
        Mode::Filter => draw_filter_bar(f, &app.input_buf, chunks[2]),
        _ => {}
    }

    layout
}

fn help_line<'a>(app: &App) -> Line<'a> {
    let help = match (&app.tab, &app.mode) {
        (_, Mode::QuickCapture) => " Enter:save  Esc:cancel ",
        (_, Mode::Filter) => " Enter/Esc:close filter ",
        (Tab::Today, _) => " q:quit  Tab:nav  Enter:chat  w:assistant  g:end-of-day  r:refresh ",
        (Tab::Tasks, _) => " q:quit  /:filter  a:approve  d:reject  e:edit  x:close  u:undo  c:capture  +/-:pri ",
        (Tab::Search, _) => " q:quit  Tab:nav  /:search  Enter:chat ",
        (Tab::Schedule, _) => " q:quit  Tab:nav  r:rerun  f:fetch-all  o:organize  Enter:run ",
        _ => " q:quit  Tab:nav  r:refresh ",
    };
    Line::from(Span::styled(help, Style::default().fg(Color::DarkGray)))
}

fn draw_filter_bar(f: &mut Frame, buf: &str, area: Rect) {
    let para = Paragraph::new(Line::from(vec![
        Span::styled(" /", Style::default().fg(Color::Cyan)),
        Span::raw(buf),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]));
    f.render_widget(para, area);
}

fn draw_capture_dialog(f: &mut Frame, buf: &str) {
    let area = centered_rect(60, 5, f.area());
    f.render_widget(Clear, area);
    let block = Block::default().borders(Borders::ALL)
        .title(" Quick Capture → Inbox (Enter to save, Esc cancel) ")
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let para = Paragraph::new(Line::from(vec![
        Span::styled(" > ", Style::default().fg(Color::Cyan)),
        Span::raw(buf),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]));
    f.render_widget(para, inner);
}

fn draw_input_dialog(f: &mut Frame, field: &InputField, agent: &str, interval: &str, prompt: &str, buf: &str) {
    let area = centered_rect(50, 11, f.area());
    f.render_widget(Clear, area);
    let block = Block::default().borders(Borders::ALL)
        .title(" Add Schedule (Enter to confirm, Esc to cancel) ")
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let highlight = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let lines = vec![
        Line::from(vec![
            Span::styled(" Agent:    ", if *field == InputField::Agent { highlight } else { dim }),
            Span::raw(if *field == InputField::Agent { buf } else { agent }),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Interval: ", if *field == InputField::Interval { highlight } else { dim }),
            Span::raw(if *field == InputField::Interval { buf } else { interval }),
            Span::styled(" (minutes)", dim),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Prompt:   ", if *field == InputField::Prompt { highlight } else { dim }),
            Span::raw(if *field == InputField::Prompt { buf } else { prompt }),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn due_style(due: Option<&str>) -> Style {
    let Some(d) = due else { return Style::default().fg(Color::DarkGray) };
    let today = Local::now().date_naive();
    match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
        Ok(date) if date < today => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        Ok(date) if date == today => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn draw_tasks(f: &mut Frame, app: &App, area: Rect) {
    let tasks = app.filtered_tasks();
    let mut by_project: Vec<(&str, Vec<(usize, &&Task)>)> = Vec::new();
    let mut project_map: HashMap<&str, usize> = HashMap::new();
    for (i, t) in tasks.iter().enumerate() {
        let proj = if t.project.is_empty() { "Ungrouped" } else { &t.project };
        if let Some(&idx) = project_map.get(proj) {
            by_project[idx].1.push((i, t));
        } else {
            project_map.insert(proj, by_project.len());
            by_project.push((proj, vec![(i, t)]));
        }
    }
    by_project.sort_by_key(|(p, _)| *p);

    let mut rows: Vec<Row> = Vec::new();
    for (proj, group) in &by_project {
        rows.push(Row::new(vec![
            Cell::from(""), Cell::from(Span::styled(format!("── {} ──", proj), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Cell::from(""), Cell::from(""), Cell::from(""),
        ]));
        for &(task_idx, t) in group {
            let pri_color = match t.priority.as_str() { "1" => Color::Red, "2" => Color::LightRed, "3" => Color::Yellow, _ => Color::White };
            let blocked = if !t.blocked_by.is_empty() { format!("⛔ {}", t.blocked_by.join(",")) } else { String::new() };
            let style = if task_idx == app.selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
            let title = if t.is_pending() {
                format!("⏳ {}", t.title.chars().take(48).collect::<String>())
            } else {
                t.title.chars().take(50).collect::<String>()
            };
            let status_style = if t.is_pending() {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            rows.push(Row::new(vec![
                Cell::from(Span::styled(&t.priority, Style::default().fg(pri_color))),
                Cell::from(title),
                Cell::from(Span::styled(t.due.as_deref().unwrap_or("-"), due_style(t.due.as_deref()))),
                Cell::from(Span::styled(&*t.status, status_style)),
                Cell::from(Span::styled(blocked, Style::default().fg(Color::Red))),
            ]).style(style));
        }
    }

    let pending = tasks.iter().filter(|t| t.is_pending()).count();
    let pending_label = if pending > 0 { format!(" ⏳ {} pending ", pending) } else { String::new() };
    let title = if app.filter.is_empty() {
        format!(" Tasks ({}){} ", tasks.len(), pending_label)
    } else {
        format!(" Tasks ({}){}[filter: {}] ", tasks.len(), pending_label, app.filter)
    };
    let table = Table::new(rows, [Constraint::Length(3), Constraint::Min(25), Constraint::Length(12), Constraint::Length(11), Constraint::Length(20)])
        .header(Row::new(vec!["Pri", "Task", "Due", "Status", "Blocked By"]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(table, area);
}

pub fn load_daily_note() -> String {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let path = std::path::Path::new(vault::vault_path()).join(format!("0. Inbox/Daily/{}.md", today));
    std::fs::read_to_string(path).unwrap_or_else(|_| "No daily note yet. Press Enter to chat with your assistant.".to_string())
}

pub fn today_logged() -> bool {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let month_file = Local::now().format("%Y-%m").to_string();
    let path = std::path::Path::new(vault::vault_path()).join(format!(".timetracking/{}.md", month_file));
    std::fs::read_to_string(path).map(|c| c.contains(&today)).unwrap_or(false)
}

fn draw_today(f: &mut Frame, app: &App, area: Rect) {
    let sections = parse_daily_sections(&app.daily_note);

    let chunks = Layout::default().direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2 + sections.meetings.len() as u16),
            Constraint::Min(6),
            Constraint::Length(2 + sections.waiting.len().min(5) as u16),
        ])
        .split(area);

    // Meetings
    let meeting_lines: Vec<Line> = sections.meetings.iter().map(|m| {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(m, Style::default().fg(Color::Yellow)),
        ])
    }).collect();
    let meetings = Paragraph::new(meeting_lines)
        .block(Block::default().borders(Borders::ALL).title(" 📅 Meetings "));
    f.render_widget(meetings, chunks[0]);

    // Tasks
    let task_lines: Vec<Line> = sections.tasks.iter().enumerate().map(|(i, t)| {
        let is_selected = i == app.selected;
        let (icon, icon_style) = if t.done {
            ("✓", Style::default().fg(Color::DarkGray))
        } else {
            ("○", Style::default().fg(Color::White))
        };
        let pri_style = match t.priority.as_str() {
            "P1" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            "P2" => Style::default().fg(Color::Yellow),
            "P3" => Style::default().fg(Color::DarkGray),
            _ => Style::default().fg(Color::White),
        };
        let title_style = if t.done {
            Style::default().fg(Color::DarkGray)
        } else if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let bg = if is_selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
        Line::from(vec![
            Span::styled(if is_selected { " ▸ " } else { "   " }, bg),
            Span::styled(icon, icon_style.patch(bg)),
            Span::styled(" ", bg),
            Span::styled(&t.priority, pri_style.patch(bg)),
            Span::styled(" ", bg),
            Span::styled(&t.title, title_style.patch(bg)),
            Span::styled(if !t.section.is_empty() { format!("  ({})", t.section) } else { String::new() }, Style::default().fg(Color::DarkGray).patch(bg)),
        ])
    }).collect();
    let tasks = Paragraph::new(task_lines)
        .block(Block::default().borders(Borders::ALL).title(format!(" Tasks ({}) ", sections.tasks.iter().filter(|t| !t.done).count())));
    f.render_widget(tasks, chunks[1]);

    // Waiting
    let wait_lines: Vec<Line> = sections.waiting.iter().map(|w| {
        Line::from(Span::styled(format!("  ⏳ {}", w), Style::default().fg(Color::DarkGray)))
    }).collect();
    let waiting = Paragraph::new(wait_lines)
        .block(Block::default().borders(Borders::ALL).title(" Waiting For "));
    f.render_widget(waiting, chunks[2]);
}

struct DailyTask {
    title: String,
    priority: String,
    done: bool,
    section: String,
}

struct DailySections {
    meetings: Vec<String>,
    tasks: Vec<DailyTask>,
    waiting: Vec<String>,
}

fn parse_daily_sections(note: &str) -> DailySections {
    let mut meetings = Vec::new();
    let mut tasks = Vec::new();
    let mut waiting = Vec::new();
    let mut current_section = String::new();

    for line in note.lines() {
        if line.starts_with("## ") || line.starts_with("### ") {
            current_section = line.trim_start_matches('#').trim().to_string();
            continue;
        }
        if current_section == "Meetings" {
            if line.starts_with("- ") {
                meetings.push(line[2..].to_string());
            }
        } else if current_section == "Waiting For" {
            if line.starts_with("- ") {
                waiting.push(line[2..].to_string());
            }
        } else if line.starts_with("- [ ]") || line.starts_with("- [x]") {
            let done = line.starts_with("- [x]");
            let raw = if done { &line[6..] } else { &line[6..] };
            // Extract priority
            let priority = if raw.contains("— P1") { "P1" }
                else if raw.contains("— P2") { "P2" }
                else if raw.contains("— P3") { "P3" }
                else { "  " };
            // Clean title: remove wiki links, URLs, priority markers
            let title = raw.split("([[").next().unwrap_or(raw)
                .split(" — P").next().unwrap_or(raw)
                .trim().to_string();
            tasks.push(DailyTask { title, priority: priority.to_string(), done, section: current_section.clone() });
        }
    }
    DailySections { meetings, tasks, waiting }
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search input
    let search_display = if app.mode == Mode::Filter { &app.input_buf } else { &app.filter };
    let input = Paragraph::new(Line::from(vec![
        Span::styled(" 🔍 ", Style::default().fg(Color::Cyan)),
        Span::raw(search_display),
        if app.mode == Mode::Filter { Span::styled("█", Style::default().fg(Color::Cyan)) } else { Span::raw("") },
    ])).block(Block::default().borders(Borders::ALL).title(" Search (/ to type) "));
    f.render_widget(input, chunks[0]);

    // Results
    if app.search_results.is_empty() {
        let msg = if app.filter.is_empty() { " Type / to search your history" } else { " No results" };
        let para = Paragraph::new(msg).block(Block::default().borders(Borders::ALL).title(" Results "));
        f.render_widget(para, chunks[1]);
    } else {
        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in app.search_results.iter().enumerate() {
            let style = if i == app.selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
            let header_style = if i == app.selected { Style::default().fg(Color::Cyan).bg(Color::DarkGray) } else { Style::default().fg(Color::Cyan) };
            lines.push(Line::from(Span::styled(
                format!(" [{}] {} | {}", r.source, r.date, r.participants), header_style,
            )));
            let snippet: String = r.content.lines().take(3).collect::<Vec<_>>().join(" ");
            lines.push(Line::from(Span::styled(
                format!("   {}", snippet.chars().take(100).collect::<String>()), style,
            )));
            lines.push(Line::from(""));
        }
        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(format!(" Results ({}) ", app.search_results.len())));
        f.render_widget(para, chunks[1]);
    }
}

fn draw_schedule(f: &mut Frame, app: &App, area: Rect) {
    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spin_char = SPINNER[(app.tick as usize) % SPINNER.len()];

    let native_count = app.native_schedules.len();
    let mut rows: Vec<Row> = Vec::new();

    // Native schedules header
    rows.push(Row::new(vec![
        Cell::from(Span::styled("Name", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Freq", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Next", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Last Run", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("", Style::default())),
    ]));

    // Native schedule rows
    for (i, ns) in app.native_schedules.iter().enumerate() {
        let status = if ns.running {
            format!("{} running", spin_char)
        } else {
            ns.last_run.map_or("pending".to_string(), |lr| {
                let next_at = lr + chrono::Duration::minutes(ns.interval_minutes as i64);
                if next_at < Local::now() { "overdue".to_string() }
                else { format!("in {}m", (next_at - Local::now()).num_minutes()) }
            })
        };
        let last = ns.last_run.map_or("-".to_string(), |lr| lr.format("%H:%M:%S").to_string());
        let style = if app.selected == i { Style::default().bg(Color::DarkGray) } else { Style::default() };
        rows.push(Row::new(vec![
            Cell::from(ns.name.clone()),
            Cell::from(format!("{}m", ns.interval_minutes)),
            Cell::from(status),
            Cell::from(last),
            Cell::from(""),
        ]).style(style));
    }

    // Divider
    rows.push(Row::new(vec![Cell::from(Span::styled(
        "─── Agents ─────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    ))]));

    // Agentic schedules header
    rows.push(Row::new(vec![
        Cell::from(Span::styled("Agent", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Trigger", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Next", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Last Run", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Prompt", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
    ]));

    // Agentic schedule rows
    for (i, s) in app.schedules.iter().enumerate() {
        let sel_idx = native_count + i;
        let status = if app.running_indices.contains(&i) {
            format!("{} running", spin_char)
        } else {
            s.last_run.map_or("pending".to_string(), |lr| {
                let next_at = lr + chrono::Duration::minutes(s.interval_minutes as i64);
                if s.interval_minutes == 0 { "on-dep".to_string() }
                else if next_at < Local::now() { "overdue".to_string() }
                else { format!("in {}m", (next_at - Local::now()).num_minutes()) }
            })
        };
        let last = s.last_run.map_or("-".to_string(), |lr| lr.format("%H:%M:%S").to_string());
        let style = if app.selected == sel_idx { Style::default().bg(Color::DarkGray) } else { Style::default() };
        rows.push(Row::new(vec![
            Cell::from(s.agent.clone()),
            Cell::from(if s.interval_minutes > 0 { format!("{}m", s.interval_minutes) } else { "dep".to_string() }),
            Cell::from(status), Cell::from(last),
            Cell::from(s.prompt.chars().take(30).collect::<String>()),
        ]).style(style));
    }

    let table = Table::new(rows, [Constraint::Length(18), Constraint::Length(8), Constraint::Length(10), Constraint::Length(10), Constraint::Min(20)])
        .block(Block::default().borders(Borders::ALL).title(" Schedules "));
    f.render_widget(table, area);
}

fn draw_people(f: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app.people.iter().enumerate().map(|(i, p)| {
        let file_marker = if p.has_file { "📄" } else { "  " };
        let ids = p.identifiers.iter().map(|(svc, handle)| format!("{}:{}", svc, handle)).collect::<Vec<_>>().join(" ");
        let waiting = if p.waiting_on.is_empty() { String::new() } else { format!("{} items", p.waiting_on.len()) };
        let style = if i == app.selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
        Row::new(vec![
            Cell::from(file_marker), Cell::from(Span::styled(&p.name, Style::default().fg(Color::Cyan))),
            Cell::from(ids), Cell::from(waiting),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, [Constraint::Length(3), Constraint::Length(20), Constraint::Min(30), Constraint::Length(10)])
        .header(Row::new(vec!["", "Name", "Identifiers", "Waiting"]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title(format!(" People ({}) ", app.people.len())));
    f.render_widget(table, area);
}

fn draw_feeds(f: &mut Frame, app: &App, area: Rect) {
    if app.feeds.is_empty() {
        let para = Paragraph::new(" No feeds found").block(Block::default().borders(Borders::ALL).title(" Feeds "));
        f.render_widget(para, area); return;
    }
    let constraints: Vec<Constraint> = app.feeds.iter().map(|_| Constraint::Ratio(1, app.feeds.len() as u32)).collect();
    let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints).split(area);
    for (i, feed) in app.feeds.iter().enumerate() {
        let title = format!(" {} (fetched {}) ", feed.source, feed.fetched);
        let lines: Vec<Line> = feed.body.lines().map(|l| {
            if l.starts_with("## ") { Line::from(Span::styled(l, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))) }
            else if l.starts_with("- **") { Line::from(Span::styled(l, Style::default().fg(Color::White))) }
            else { Line::from(Span::styled(l, Style::default().fg(Color::DarkGray))) }
        }).collect();
        let para = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(para, chunks[i]);
    }
}

fn draw_agents(f: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app.agents.iter().enumerate().map(|(i, a)| {
        let style = if i == app.selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
        Row::new(vec![
            Cell::from(Span::styled(&a.name, Style::default().fg(Color::Cyan))),
            Cell::from(a.description.chars().take(60).collect::<String>()),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, [Constraint::Length(22), Constraint::Min(30)])
        .header(Row::new(vec!["Agent", "Description"]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title(format!(" Agents ({}) — Enter to chat ", app.agents.len())));
    f.render_widget(table, area);
}

fn draw_chat(f: &mut Frame, app: &App) {
    let chunks = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let status = if app.chat_streaming { " ⠋ thinking..." } else { "" };
    let chat_title = if app.chat_agent.is_empty() { "Chat".to_string() } else { format!("Chat: {}", app.chat_agent) };
    let title = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {}", chat_title), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(status, Style::default().fg(Color::Yellow)),
        Span::styled("  (Esc to close)", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(title, chunks[0]);

    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.chat_messages {
        let (prefix, style) = match msg.role {
            ChatRole::User => ("▶ ", Style::default().fg(Color::Green)),
            ChatRole::Agent => ("", Style::default().fg(Color::White)),
            ChatRole::System => ("⚙ ", Style::default().fg(Color::DarkGray)),
        };
        for line in msg.text.lines() {
            lines.push(Line::from(Span::styled(format!("{}{}", prefix, line), style)));
        }
        lines.push(Line::from(""));
    }
    let visible_height = chunks[1].height as usize;
    let max_scroll = if lines.len() > visible_height { (lines.len() - visible_height) as u16 } else { 0 };
    let scroll = if app.chat_scroll_offset == 0 { max_scroll } else { max_scroll.saturating_sub(app.chat_scroll_offset) };
    let messages = Paragraph::new(lines).scroll((scroll, 0)).block(Block::default().borders(Borders::ALL));
    f.render_widget(messages, chunks[1]);

    let input = Paragraph::new(Line::from(vec![
        Span::styled(" > ", Style::default().fg(Color::Cyan)),
        Span::raw(&app.input_buf),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ])).block(Block::default().borders(Borders::ALL).title(" Send (Enter) "));
    f.render_widget(input, chunks[2]);
}
