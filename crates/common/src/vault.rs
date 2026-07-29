use std::fs;
use std::path::Path;

pub fn vault_path() -> &'static str {
    &crate::config::get().general.vault_path
}

/// Task awaiting user approval — created from a kb record but not yet active.
pub const STATUS_PENDING: &str = "pending";
/// Approved (active) task.
pub const STATUS_OPEN: &str = "open";
/// Pending task the user declined. The file is kept so the same kb record is
/// not proposed again on the next inbox run, but it is hidden from task lists.
pub const STATUS_REJECTED: &str = "rejected";
/// Completed task.
pub const STATUS_DONE: &str = "done";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub title: String,
    pub status: String,
    pub priority: String,
    pub project: String,
    pub due: Option<String>,
    pub file: String,
    pub id: String,
    pub blocked_by: Vec<String>,
    pub source_url: Option<String>,
}

impl Task {
    /// True when this task still needs approval before it counts as active work.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.status == STATUS_PENDING
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InboxItem {
    pub text: String,
    pub checked: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WaitingItem {
    pub date: String,
    pub person: String,
    pub item: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Feed {
    pub source: String,
    pub fetched: String,
    pub body: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Person {
    pub name: String,
    pub has_file: bool,
    pub waiting_on: Vec<String>,
    pub tasks: Vec<String>,
    pub identifiers: Vec<(String, String)>,
}

// --- Frontmatter helpers ---
//
// The vault convention is single-line YAML frontmatter (`key: value`), which is
// what these helpers read and write. Multi-line/block values are not handled.

/// Read a single-line frontmatter value, without surrounding quotes.
#[must_use]
pub fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let fm = content.strip_prefix("---")?.split_once("---")?.0;
    let prefix = format!("{key}:");
    fm.lines()
        .find(|l| l.starts_with(&prefix))
        .map(|l| l[prefix.len()..].trim().trim_matches('"').to_string())
}

/// True when the frontmatter key holds exactly `value`.
#[must_use]
pub fn has_frontmatter_value(content: &str, key: &str, value: &str) -> bool {
    frontmatter_value(content, key).is_some_and(|v| v == value)
}

/// Upsert a frontmatter key, preserving the rest of the file byte-for-byte.
/// Returns `content` unchanged when there is no frontmatter block to write into.
#[must_use]
pub fn set_frontmatter(content: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return content.to_string();
    }
    let Some(end) = lines.iter().skip(1).position(|l| l.trim_end() == "---").map(|i| i + 1) else {
        return content.to_string();
    };

    let prefix = format!("{key}:");
    if let Some(existing) = lines[1..end].iter_mut().find(|l| l.starts_with(&prefix)) {
        *existing = format!("{key}: {value}");
    } else {
        lines.insert(end, format!("{key}: {value}"));
    }

    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Kanban column tags, in board order. Shared by the Obsidian Kanban board and
/// the Trello list mapping.
pub const COLUMN_TAGS: [&str; 4] = ["today", "this-week", "later", "backlog"];

fn parse_tag_list(raw: &str) -> Vec<String> {
    raw.trim()
        .trim_matches(&['[', ']'] as &[char])
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_start_matches('#').to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The column tag currently assigned to a task, if any.
#[must_use]
pub fn column_tag(content: &str) -> Option<String> {
    let tags = parse_tag_list(&frontmatter_value(content, "tags").unwrap_or_default());
    COLUMN_TAGS.iter().find(|c| tags.iter().any(|t| t == *c)).map(|c| (*c).to_string())
}

/// Replace the task's column tag, leaving any non-column tags in place.
#[must_use]
pub fn set_column_tag(content: &str, tag: &str) -> String {
    let mut tags: Vec<String> = parse_tag_list(&frontmatter_value(content, "tags").unwrap_or_default())
        .into_iter()
        .filter(|t| !COLUMN_TAGS.contains(&t.as_str()))
        .collect();
    tags.insert(0, tag.to_string());
    set_frontmatter(content, "tags", &format!("[{}]", tags.join(", ")))
}

pub fn load_tasks() -> Vec<Task> {
    let dir = Path::new(vault_path()).join("Tasks");
    let mut tasks = vec![];
    let Ok(entries) = fs::read_dir(&dir) else { return tasks };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            if let Some(t) = parse_task(&path) {
                tasks.push(t);
            }
        }
    }
    tasks.sort_by(|a, b| a.priority.cmp(&b.priority));
    tasks
}

fn parse_task(path: &Path) -> Option<Task> {
    let content = fs::read_to_string(path).ok()?;
    let fm = content.strip_prefix("---")?.split_once("---")?.0;

    let get = |key: &str| -> String {
        fm.lines()
            .find(|l| l.starts_with(key))
            .map(|l| l[key.len()..].trim().trim_matches('"').to_string())
            .unwrap_or_default()
    };

    let status = get("status:");
    if status == STATUS_DONE || status == STATUS_REJECTED {
        return None;
    }

    let blocked_by: Vec<String> = fm.lines()
        .find(|l| l.starts_with("blocked_by:") || l.starts_with("dependsOn:"))
        .map(|l| {
            let colon_pos = l.find(':').unwrap_or(0);
            let val = l[colon_pos + 1..].trim();
            val.trim_matches(&['[', ']'] as &[char])
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Some(Task {
        title: get("title:"),
        status,
        priority: get("priority:"),
        project: get("project:").replace("[[", "").replace("]]", ""),
        due: {
            let d = get("due:");
            if d.is_empty() { None } else { Some(d) }
        },
        file: path.file_name()?.to_string_lossy().to_string(),
        id: get("id:"),
        blocked_by,
        source_url: {
            let u = get("source_url:");
            if u.is_empty() { None } else { Some(u) }
        },
    })
}

pub fn load_inbox() -> Vec<InboxItem> {
    let path = Path::new(vault_path()).join("0. Inbox/Inbox.md");
    let Ok(content) = fs::read_to_string(&path) else { return vec![] };

    content
        .lines()
        .filter(|l| l.starts_with("- [") || l.starts_with("## "))
        .map(|l| {
            if l.starts_with("## ") {
                InboxItem { text: l[3..].to_string(), checked: false }
            } else {
                let checked = l.starts_with("- [x]");
                let text = l.get(6..).unwrap_or("").to_string();
                InboxItem { text, checked }
            }
        })
        .collect()
}

pub fn load_waiting() -> Vec<WaitingItem> {
    let path = Path::new(vault_path()).join("Waiting For.md");
    let Ok(content) = fs::read_to_string(&path) else { return vec![] };

    content
        .lines()
        .filter(|l| l.starts_with("| 20"))
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('|').map(|s| s.trim()).collect();
            if cols.len() >= 4 {
                Some(WaitingItem {
                    date: cols[1].to_string(),
                    person: cols[2].to_string(),
                    item: cols[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn today_daily_exists() -> bool {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    Path::new(vault_path())
        .join(format!("0. Inbox/Daily/{}.md", today))
        .exists()
}

pub fn update_task_priority(task: &Task, new_priority: &str) {
    let path = Path::new(vault_path()).join("Tasks").join(&task.file);
    let Ok(content) = fs::read_to_string(&path) else { return };
    let updated = if content.contains("\npriority:") {
        content.lines().map(|l| {
            if l.starts_with("priority:") {
                format!("priority: {}", new_priority)
            } else {
                l.to_string()
            }
        }).collect::<Vec<_>>().join("\n")
    } else {
        content.lines().map(|l| {
            if l.starts_with("status:") {
                format!("{}\npriority: {}", l, new_priority)
            } else {
                l.to_string()
            }
        }).collect::<Vec<_>>().join("\n")
    };
    fs::write(path, updated).ok();
}

/// Rewrite the `status:` frontmatter line of a task file in place.
fn rewrite_status(task: &Task, new_status: &str) {
    let path = Path::new(vault_path()).join("Tasks").join(&task.file);
    let Ok(content) = fs::read_to_string(&path) else { return };
    let updated = content.lines().map(|l| {
        if l.starts_with("status:") {
            format!("status: {}", new_status)
        } else {
            l.to_string()
        }
    }).collect::<Vec<_>>().join("\n");
    fs::write(path, updated).ok();
}

pub fn close_task(task: &Task) {
    rewrite_status(task, STATUS_DONE);
}

/// Restore a task's previous status (undo for close/approve/reject).
pub fn reopen_task(task: &Task) {
    rewrite_status(task, &task.status);
}

/// Approve a pending task: it becomes active work.
pub fn approve_task(task: &Task) {
    rewrite_status(task, STATUS_OPEN);
}

/// Reject a pending task: hidden from task lists and not proposed again.
pub fn reject_task(task: &Task) {
    rewrite_status(task, STATUS_REJECTED);
}

pub fn add_to_inbox(text: &str) {
    let path = Path::new(vault_path()).join("0. Inbox/Inbox.md");
    let mut content = fs::read_to_string(&path).unwrap_or_default();
    content.push_str(&format!("\n- [ ] {}", text));
    fs::write(path, content).ok();
}

// --- Frontmatter helpers (continued) ---

/// Remove a frontmatter key if present.
#[must_use]
pub fn remove_frontmatter(content: &str, key: &str) -> String {
    let Some(end) = frontmatter_end(content) else { return content.to_string() };
    let prefix = format!("{}:", key);
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    if let Some(pos) = lines[1..end].iter().position(|l| l.starts_with(&prefix)) {
        lines.remove(pos + 1);
    }
    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Index of the closing `---` line of the frontmatter block, if the file has one.
fn frontmatter_end(content: &str) -> Option<usize> {
    let mut lines = content.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    lines.position(|l| l.trim_end() == "---").map(|i| i + 1)
}

/// Recursively collect `.md` files under `dir`, skipping Kanban board files and
/// `_`-prefixed files (templates and plugin state, not content).
#[must_use]
pub fn walk_md_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return files };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk_md_files(&path));
        } else if path.extension().is_some_and(|e| e == "md") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with("Kanban") && !name.starts_with('_') {
                files.push(path);
            }
        }
    }
    files
}


pub fn load_feeds() -> Vec<Feed> {
    let dir = Path::new(vault_path()).join(".feeds");
    let Ok(entries) = fs::read_dir(&dir) else { return vec![] };
    let mut feeds: Vec<Feed> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| {
            let content = fs::read_to_string(e.path()).ok()?;
            let (source, fetched, body) = parse_feed_frontmatter(&content);
            Some(Feed { source, fetched, body })
        })
        .collect();
    feeds.sort_by(|a, b| b.fetched.cmp(&a.fetched));
    feeds
}

pub fn load_people() -> Vec<Person> {
    let mut people: std::collections::HashMap<String, Person> = std::collections::HashMap::new();

    for w in load_waiting() {
        let name = w.person.trim().to_string();
        if name.is_empty() { continue; }
        let entry = people.entry(name.clone()).or_insert_with(|| Person {
            name: name.clone(), has_file: false, waiting_on: vec![], tasks: vec![], identifiers: vec![],
        });
        entry.waiting_on.push(w.item);
    }

    let dir = Path::new(vault_path()).join("Tasks");
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(fm) = content.strip_prefix("---").and_then(|r| r.split_once("---").map(|(f, _)| f)) {
                        let title = fm.lines().find(|l| l.starts_with("title:"))
                            .map(|l| l["title:".len()..].trim().trim_matches('"').to_string())
                            .unwrap_or_default();
                        if let Some(line) = fm.lines().find(|l| l.starts_with("waiting_for:")) {
                            let name = line["waiting_for:".len()..].trim()
                                .trim_matches('"').replace("[[People/", "").replace("]]", "").to_string();
                            if !name.is_empty() {
                                let entry = people.entry(name.clone()).or_insert_with(|| Person {
                                    name: name.clone(), has_file: false, waiting_on: vec![], tasks: vec![], identifiers: vec![],
                                });
                                entry.tasks.push(title);
                            }
                        }
                    }
                }
            }
        }
    }

    let people_dir = Path::new(vault_path()).join("People");
    if let Ok(entries) = fs::read_dir(&people_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                if name == "Me" { continue; }
                let person = people.entry(name.clone()).or_insert_with(|| Person {
                    name: name.clone(), has_file: false, waiting_on: vec![], tasks: vec![], identifiers: vec![],
                });
                person.has_file = true;
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(fm) = content.strip_prefix("---").and_then(|r| r.split_once("---").map(|(f, _)| f)) {
                        for key in &["gitlab:", "slack:", "linear:", "email:", "slack_id:"] {
                            if let Some(line) = fm.lines().find(|l| l.starts_with(key)) {
                                let val = line[key.len()..].trim().trim_matches('"').to_string();
                                if !val.is_empty() {
                                    person.identifiers.push((key.trim_end_matches(':').to_string(), val));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut result: Vec<Person> = people.into_values().collect();
    result.sort_by(|a, b| b.waiting_on.len().cmp(&a.waiting_on.len()).then(a.name.cmp(&b.name)));
    result
}

fn parse_feed_frontmatter(content: &str) -> (String, String, String) {
    let mut source = String::new();
    let mut fetched = String::new();
    let body;

    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some((fm, after)) = rest.split_once("\n---\n") {
            for line in fm.lines() {
                if let Some(v) = line.strip_prefix("source:") {
                    source = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("fetched:") {
                    fetched = v.trim().to_string();
                }
            }
            body = after.to_string();
        } else {
            body = content.to_string();
        }
    } else {
        body = content.to_string();
    }
    (source, fetched, body)
}

// --- Triage Patterns ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriagePattern {
    pub source: String,
    pub keywords: Vec<String>,
    pub column: String,
    pub hits: u32,
    pub misses: u32,
    pub last_updated: String,
}

pub fn triage_patterns_path() -> std::path::PathBuf {
    Path::new(vault_path()).join("0. Inbox/triage-patterns.json")
}

pub fn load_triage_patterns() -> Vec<TriagePattern> {
    let path = triage_patterns_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_triage_patterns(patterns: &[TriagePattern]) {
    let path = triage_patterns_path();
    if let Ok(json) = serde_json::to_string_pretty(patterns) {
        fs::write(path, json).ok();
    }
}

pub fn suggest_columns(tasks: &[Task], patterns: &[TriagePattern]) -> Vec<(String, Option<(String, u32, u32)>)> {
    let column_tags = ["today", "this-week", "later", "backlog"];
    let untagged: Vec<&Task> = tasks.iter()
        .filter(|t| t.status != STATUS_DONE && !t.is_pending())
        .filter(|t| !column_tags.iter().any(|col| t.title.to_lowercase().contains(col)))
        .collect();

    untagged.iter().map(|task| {
        let title_words: Vec<String> = task.title.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(|s| s.to_string())
            .collect();

        let suggestion = patterns.iter()
            .filter(|p| p.hits >= 3 && (p.misses as f32 / (p.hits + p.misses) as f32) < 0.3)
            .find(|p| {
                let source_match = task.file.to_lowercase().contains(&p.source) ||
                    task.title.to_lowercase().contains(&p.source);
                let keyword_match = p.keywords.iter().any(|kw| title_words.contains(&kw.to_lowercase()));
                source_match && keyword_match
            })
            .map(|p| (p.column.clone(), p.hits, p.hits + p.misses));

        (task.title.clone(), suggestion)
    }).collect()
}
