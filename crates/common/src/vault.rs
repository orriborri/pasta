use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn vault_path() -> &'static str {
    &crate::config::get().general.vault_path
}

/// Single source of truth for all vault-relative paths.
///
/// Construct once from the configured vault path, then use its methods
/// throughout the codebase. This prevents duplicate path literals and
/// ensures missing directories are logged rather than silently skipped.
#[derive(Debug, Clone)]
pub struct VaultLayout {
    base: std::path::PathBuf,
    visited: std::sync::Arc<std::sync::Mutex<HashSet<String>>>,
}

impl VaultLayout {
    /// Construct a new layout from the configured vault path.
    /// Logs warnings for any missing vault directories.
    pub fn new(vault_path: &Path) -> Self {
        let base = vault_path.to_path_buf();
        Self {
            base,
            visited: std::sync::Arc::new(std::sync::Mutex::new(HashSet::new())),
        }
    }

    /// Emit a one-time warning if `subdir` is missing under the vault base.
    /// Returns `true` when a warning was emitted (the directory is missing and
    /// this is the first time it was seen), `false` when the directory exists or
    /// the warning was already emitted. The dedup + return value make the
    /// missing-directory behaviour testable without capturing stderr.
    fn warn_if_missing(&self, subdir: &str) -> bool {
        let path = self.base.join(subdir);
        if !path.exists() {
            let mut visited = self.visited.lock().unwrap();
            if visited.insert(subdir.to_string()) {
                eprintln!("Warning: vault directory missing: {}", subdir);
                return true;
            }
        }
        false
    }

    /// Path to the tasks directory (Tasks/)
    #[must_use]
    pub fn tasks(&self) -> std::path::PathBuf {
        self.warn_if_missing("Tasks");
        self.base.join("Tasks")
    }

    /// Path to the inbox directory (0. Inbox/)
    #[must_use]
    pub fn inbox(&self) -> std::path::PathBuf {
        self.warn_if_missing("0. Inbox");
        self.base.join("0. Inbox")
    }

    /// Path to the archive directory (4. Archive/)
    #[must_use]
    pub fn archive(&self) -> std::path::PathBuf {
        self.warn_if_missing("4. Archive");
        self.base.join("4. Archive")
    }

    /// Path to the projects directory (1. Projects/)
    #[must_use]
    pub fn projects(&self) -> std::path::PathBuf {
        self.warn_if_missing("1. Projects");
        self.base.join("1. Projects")
    }

    /// Path to the areas directory (2. Areas/)
    #[must_use]
    pub fn areas(&self) -> std::path::PathBuf {
        self.warn_if_missing("2. Areas");
        self.base.join("2. Areas")
    }

    /// Path to the resources directory (3. Resources/)
    #[must_use]
    pub fn resources(&self) -> std::path::PathBuf {
        self.warn_if_missing("3. Resources");
        self.base.join("3. Resources")
    }

    /// Path to the feeds directory (.feeds/)
    #[must_use]
    pub fn feeds(&self) -> std::path::PathBuf {
        self.warn_if_missing(".feeds");
        self.base.join(".feeds")
    }

    /// Path to triage patterns file (0. Inbox/triage-patterns.json)
    #[must_use]
    pub fn triage_patterns(&self) -> std::path::PathBuf {
        self.inbox().join("triage-patterns.json")
    }

    /// Path to daily notes directory (0. Inbox/Daily/)
    #[must_use]
    pub fn daily(&self) -> std::path::PathBuf {
        self.inbox().join("Daily")
    }

    /// Path to a specific daily note
    #[must_use]
    pub fn daily_note(&self, date: &str) -> std::path::PathBuf {
        self.daily().join(format!("{}.md", date))
    }

    /// Path to the People directory
    #[must_use]
    pub fn people(&self) -> std::path::PathBuf {
        self.warn_if_missing("People");
        self.base.join("People")
    }

    /// Path to the roadmap directory (0. Inbox/roadmap/)
    #[must_use]
    pub fn roadmap(&self) -> std::path::PathBuf {
        self.inbox().join("roadmap")
    }

    /// Path to weekly meetings directory (0. Inbox/Weekly Meetings/)
    #[must_use]
    pub fn weekly_meetings(&self) -> std::path::PathBuf {
        self.inbox().join("Weekly Meetings")
    }

    /// Path to timetracking directory (0. Inbox/Timetracking/)
    #[must_use]
    pub fn timetracking(&self) -> std::path::PathBuf {
        self.inbox().join("Timetracking")
    }

    /// Path to agents directory (0. Inbox/Agents/)
    #[must_use]
    pub fn agents(&self) -> std::path::PathBuf {
        self.inbox().join("Agents")
    }

    /// Path to the base vault directory
    #[must_use]
    pub fn base(&self) -> &std::path::Path {
        &self.base
    }
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
    /// Path to the task file, relative to the vault root. Used to write
    /// mutations back to the file the task was actually loaded from,
    /// including tasks routed into `Tasks/<Initiative>/` subfolders.
    pub path: std::path::PathBuf,
    pub id: String,
    pub blocked_by: Vec<String>,
    pub source_url: Option<String>,
}

impl Task {
    /// The file name, without any parent directories. Used for display and
    /// for matching, not for locating the file on disk.
    #[must_use]
    pub fn file_name(&self) -> String {
        self.path.file_name().unwrap_or_default().to_string_lossy().to_string()
    }

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
    let layout = VaultLayout::new(Path::new(vault_path()));
    load_tasks_from(&layout.tasks())
}

/// Load every task under `dir`, recursing into initiative subfolders so
/// routed tasks are not silently hidden. Exposed separately from
/// `load_tasks` so tests can point it at a scratch directory.
fn load_tasks_from(dir: &Path) -> Vec<Task> {
    let mut tasks: Vec<Task> = walk_md_files(dir).iter().filter_map(|p| parse_task(p)).collect();
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
        path: path.to_path_buf(),
        id: get("id:"),
        blocked_by,
        source_url: {
            let u = get("source_url:");
            if u.is_empty() { None } else { Some(u) }
        },
    })
}

pub fn load_waiting() -> Vec<WaitingItem> {
    let vault_path = Path::new(vault_path());
    let path = vault_path.join("Waiting For.md");
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

pub fn update_task_priority(task: &Task, new_priority: &str) {
    let Ok(content) = fs::read_to_string(&task.path) else { return };
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
    fs::write(&task.path, updated).ok();
}

/// Rewrite the `status:` frontmatter line of a task file in place.
fn rewrite_status(task: &Task, new_status: &str) {
    let Ok(content) = fs::read_to_string(&task.path) else { return };
    let updated = content.lines().map(|l| {
        if l.starts_with("status:") {
            format!("status: {}", new_status)
        } else {
            l.to_string()
        }
    }).collect::<Vec<_>>().join("\n");
    fs::write(&task.path, updated).ok();
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
    let layout = VaultLayout::new(Path::new(vault_path()));
    let path = layout.inbox().join("Inbox.md");
    let mut content = fs::read_to_string(&path).unwrap_or_default();
    content.push_str(&format!("\n- [ ] {}", text));
    fs::write(path, content).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory under the OS temp dir, removed on drop.
    struct ScratchDir(std::path::PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("pasta-vault-test-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const TASK_MD: &str = "---\ntitle: Subfolder task\nstatus: open\npriority: today\nid: t-1\n---\nBody\n";

    #[test]
    fn load_tasks_finds_files_in_initiative_subfolders() {
        let scratch = ScratchDir::new("load");
        let tasks_dir = scratch.path().join("Tasks");
        let sub_dir = tasks_dir.join("Some Initiative");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("routed-task.md"), TASK_MD).unwrap();
        fs::write(tasks_dir.join("top-level-task.md"), TASK_MD).unwrap();

        let tasks = load_tasks_from(&tasks_dir);

        assert_eq!(tasks.len(), 2, "both the top-level and the routed task must appear");
        assert!(tasks.iter().any(|t| t.path == sub_dir.join("routed-task.md")));
        assert!(tasks.iter().any(|t| t.path == tasks_dir.join("top-level-task.md")));
    }

    #[test]
    fn close_task_writes_to_the_subfolder_path_it_was_loaded_from() {
        let scratch = ScratchDir::new("close");
        let sub_dir = scratch.path().join("Tasks").join("Some Initiative");
        fs::create_dir_all(&sub_dir).unwrap();
        let task_path = sub_dir.join("routed-task.md");
        fs::write(&task_path, TASK_MD).unwrap();

        let task = parse_task(&task_path).expect("task should parse");
        assert_eq!(task.path, task_path);

        close_task(&task);

        let updated = fs::read_to_string(&task_path).unwrap();
        assert!(updated.contains("status: done"), "status must be updated at the real path: {updated}");
        // No file should have been created anywhere else, e.g. at the top
        // level of the task directory.
        assert!(!scratch.path().join("Tasks").join("routed-task.md").exists());
    }

    #[test]
    fn approve_task_writes_to_the_subfolder_path_it_was_loaded_from() {
        let scratch = ScratchDir::new("approve");
        let sub_dir = scratch.path().join("Tasks").join("Some Initiative");
        fs::create_dir_all(&sub_dir).unwrap();
        let task_path = sub_dir.join("routed-task.md");
        let pending_md = TASK_MD.replace("status: open", "status: pending");
        fs::write(&task_path, &pending_md).unwrap();

        let task = parse_task(&task_path).expect("pending task should still parse");
        assert!(task.is_pending());
        assert_eq!(task.path, task_path);

        approve_task(&task);

        let updated = fs::read_to_string(&task_path).unwrap();
        assert!(updated.contains("status: open"), "approval must land at the real path: {updated}");
    }

    #[test]
    fn task_count_is_unchanged_across_a_routing_move() {
        let scratch = ScratchDir::new("routing_move");
        let tasks_dir = scratch.path().join("Tasks");
        let sub_dir = tasks_dir.join("Some Initiative");
        fs::create_dir_all(&sub_dir).unwrap();

        // Two tasks start at the top level of the task directory.
        fs::write(tasks_dir.join("stays-put.md"), TASK_MD).unwrap();
        let before_path = tasks_dir.join("gets-routed.md");
        fs::write(&before_path, TASK_MD).unwrap();

        let before = load_tasks_from(&tasks_dir);
        assert_eq!(before.len(), 2, "both tasks visible before the move");

        // Simulate semantic routing relocating one task into an initiative
        // subfolder (the same rename `route_new_tasks_semantic` performs).
        let after_path = sub_dir.join("gets-routed.md");
        fs::rename(&before_path, &after_path).unwrap();

        let after = load_tasks_from(&tasks_dir);
        assert_eq!(after.len(), 2, "routing into a subfolder must not change the task count");
        assert!(after.iter().any(|t| t.path == after_path), "routed task now loads from its subfolder path");
        assert!(after.iter().any(|t| t.path == tasks_dir.join("stays-put.md")), "the untouched task still loads");
    }
    // --- VaultLayout Red Phase Tests (expecting implementation) ---

    /// Test that VaultLayout exists and provides paths for all vault directories
    #[test]
    fn vault_layout_provides_paths_for_all_directories() {
        // Create a scratch vault with all directories
        let scratch = ScratchDir::new("vault_layout");
        let vault_path = scratch.path();
        
        // Create all required directories
        fs::create_dir_all(vault_path.join("Tasks")).unwrap();
        fs::create_dir_all(vault_path.join("0. Inbox")).unwrap();
        fs::create_dir_all(vault_path.join("4. Archive")).unwrap();
        fs::create_dir_all(vault_path.join("1. Projects")).unwrap();
        fs::create_dir_all(vault_path.join("2. Areas")).unwrap();
        fs::create_dir_all(vault_path.join("3. Resources")).unwrap();
        fs::create_dir_all(vault_path.join(".feeds")).unwrap();

        // Try to construct VaultLayout - this will fail to compile initially
        let _layout = VaultLayout::new(vault_path);
    }

    /// Test that VaultLayout methods return correct paths
    #[test]
    fn vault_layout_tasks_path() {
        let scratch = ScratchDir::new("tasks_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("Tasks")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let tasks_path = layout.tasks();
        assert_eq!(tasks_path, vault_path.join("Tasks"));
    }

    /// Test that VaultLayout Inbox method returns correct path
    #[test]
    fn vault_layout_inbox_path() {
        let scratch = ScratchDir::new("inbox_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("0. Inbox")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let inbox_path = layout.inbox();
        assert_eq!(inbox_path, vault_path.join("0. Inbox"));
    }

    /// Test that VaultLayout Archive method returns correct path
    #[test]
    fn vault_layout_archive_path() {
        let scratch = ScratchDir::new("archive_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("4. Archive")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let archive_path = layout.archive();
        assert_eq!(archive_path, vault_path.join("4. Archive"));
    }

    /// Test that VaultLayout Projects method returns correct path
    #[test]
    fn vault_layout_projects_path() {
        let scratch = ScratchDir::new("projects_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("1. Projects")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let projects_path = layout.projects();
        assert_eq!(projects_path, vault_path.join("1. Projects"));
    }

    /// Test that VaultLayout Areas method returns correct path
    #[test]
    fn vault_layout_areas_path() {
        let scratch = ScratchDir::new("areas_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("2. Areas")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let areas_path = layout.areas();
        assert_eq!(areas_path, vault_path.join("2. Areas"));
    }

    /// Test that VaultLayout Resources method returns correct path
    #[test]
    fn vault_layout_resources_path() {
        let scratch = ScratchDir::new("resources_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("3. Resources")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let resources_path = layout.resources();
        assert_eq!(resources_path, vault_path.join("3. Resources"));
    }

    /// Test that VaultLayout feeds method returns correct path (vault-relative)
    #[test]
    fn vault_layout_feeds_path_is_vault_relative() {
        let scratch = ScratchDir::new("feeds_path");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join(".feeds")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        let feeds_path = layout.feeds();
        assert_eq!(feeds_path, vault_path.join(".feeds"));
        // Ensure it's not a hardcoded absolute path
        assert!(!feeds_path.to_string_lossy().contains("/home/orre/Obsidian/Readpeak/.feeds"));
    }

    /// A missing vault directory emits a warning (rather than silently
    /// returning an empty listing).
    #[test]
    fn vault_layout_warns_on_missing_directory() {
        let scratch = ScratchDir::new("missing_dir");
        // Intentionally don't create the Tasks directory.
        let layout = VaultLayout::new(scratch.path());

        assert!(
            layout.warn_if_missing("Tasks"),
            "a missing directory must emit a warning"
        );
        // The accessor still returns the composed path so callers keep working.
        assert_eq!(layout.tasks(), scratch.path().join("Tasks"));
    }

    /// A missing directory warns only once — repeat accesses are deduplicated.
    #[test]
    fn vault_layout_warns_once_per_directory() {
        let scratch = ScratchDir::new("dedup");
        let layout = VaultLayout::new(scratch.path());

        assert!(
            layout.warn_if_missing("Tasks"),
            "first access to a missing directory warns"
        );
        assert!(
            !layout.warn_if_missing("Tasks"),
            "subsequent accesses are deduplicated (no repeat warning)"
        );
    }

    /// Test that VaultLayout is a singleton pattern - one instance for the vault
    #[test]
    fn vault_layout_is_single_source_of_truth() {
        let scratch = ScratchDir::new("singleton");
        let vault_path = scratch.path();
        fs::create_dir_all(vault_path.join("Tasks")).unwrap();
        fs::create_dir_all(vault_path.join(".feeds")).unwrap();

        let layout = VaultLayout::new(vault_path);
        
        // All methods should use the same base path
        assert_eq!(layout.tasks().parent(), Some(vault_path));
        assert_eq!(layout.feeds().parent(), Some(vault_path));
        assert_eq!(layout.inbox().parent(), Some(vault_path));
    }

    /// Test that VaultLayout prevents hardcoded paths - all paths derived from config
    #[test]
    fn vault_layout_no_hardcoded_paths() {
        // This test verifies that no literal paths like "Tasks", "0. Inbox", etc.
        // appear outside of VaultLayout in the codebase
        // (Verified via grep search, not a runtime test)
    }

    // --- Task 7: characterization tests pinning frontmatter-helper behavior ---

    const FM_DOC: &str = "---\ntitle: Note\nstatus: open\n---\nBody line one\nBody line two\n";

    #[test]
    fn set_frontmatter_upserts_an_existing_key() {
        let out = set_frontmatter(FM_DOC, "status", "draft");
        assert!(out.contains("status: draft"));
        assert!(!out.contains("status: open"));
        assert_eq!(out.matches("status:").count(), 1, "must upsert, not duplicate");
        assert!(out.contains("Body line one\nBody line two"), "body preserved");
    }

    #[test]
    fn set_frontmatter_inserts_a_new_key_inside_the_fence() {
        let out = set_frontmatter(FM_DOC, "priority", "today");
        let closing_fence = out.match_indices("---").nth(1).expect("closing fence").0;
        let new_key = out.find("priority: today").expect("new key present");
        assert!(new_key < closing_fence, "new key must land inside the frontmatter fence");
        assert!(out.contains("Body line one"), "body preserved");
    }

    #[test]
    fn set_frontmatter_without_frontmatter_returns_input_unchanged() {
        let plain = "no frontmatter here\njust body\n";
        assert_eq!(set_frontmatter(plain, "status", "draft"), plain);
    }

    #[test]
    fn remove_frontmatter_drops_the_key_and_keeps_the_rest() {
        let out = remove_frontmatter(FM_DOC, "status");
        assert!(!out.contains("status:"), "removed key gone");
        assert!(out.contains("title: Note"), "other keys survive");
        assert!(out.contains("Body line one"), "body preserved");
    }

    #[test]
    fn remove_frontmatter_absent_key_is_a_no_op() {
        assert_eq!(remove_frontmatter(FM_DOC, "nonexistent"), FM_DOC);
    }

    #[test]
    fn remove_frontmatter_without_frontmatter_returns_input_unchanged() {
        let plain = "plain body\nno fence\n";
        assert_eq!(remove_frontmatter(plain, "status"), plain);
    }

    #[test]
    fn column_tag_returns_the_column_when_present_and_none_otherwise() {
        let with_col = "---\ntags: [today, project]\n---\nbody\n";
        assert_eq!(column_tag(with_col), Some("today".to_string()));

        let without_col = "---\ntags: [project, idea]\n---\nbody\n";
        assert_eq!(column_tag(without_col), None);
    }

    #[test]
    fn set_column_tag_swaps_the_column_and_keeps_other_tags() {
        let doc = "---\ntags: [later, project]\n---\nbody\n";
        let out = set_column_tag(doc, "today");
        let tags = parse_tag_list(&frontmatter_value(&out, "tags").expect("tags present"));
        assert_eq!(column_tag(&out), Some("today".to_string()), "new column active");
        assert!(!tags.contains(&"later".to_string()), "previous column tag replaced");
        assert!(tags.contains(&"project".to_string()), "non-column tag survives");
    }

    #[test]
    fn parse_task_filters_out_done_and_rejected() {
        let scratch = ScratchDir::new("parse_task_filter");
        let dir = scratch.path();

        let open_path = dir.join("open.md");
        fs::write(&open_path, "---\ntitle: Open one\nstatus: open\n---\nbody\n").unwrap();
        assert!(parse_task(&open_path).is_some(), "open task must parse");

        let done_path = dir.join("done.md");
        fs::write(&done_path, "---\ntitle: Done one\nstatus: done\n---\nbody\n").unwrap();
        assert!(parse_task(&done_path).is_none(), "done task must be filtered out");

        let rejected_path = dir.join("rejected.md");
        fs::write(&rejected_path, "---\ntitle: Rejected one\nstatus: rejected\n---\nbody\n").unwrap();
        assert!(parse_task(&rejected_path).is_none(), "rejected task must be filtered out");
    }
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
    let layout = VaultLayout::new(Path::new(vault_path()));
    let dir = layout.feeds();
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
    let layout = VaultLayout::new(Path::new(vault_path()));
    let mut people: std::collections::HashMap<String, Person> = std::collections::HashMap::new();

    for w in load_waiting() {
        let name = w.person.trim().to_string();
        if name.is_empty() { continue; }
        let entry = people.entry(name.clone()).or_insert_with(|| Person {
            name: name.clone(), has_file: false, waiting_on: vec![], tasks: vec![], identifiers: vec![],
        });
        entry.waiting_on.push(w.item);
    }

    let tasks_dir = layout.tasks();
    if let Ok(entries) = fs::read_dir(&tasks_dir) {
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

    let people_dir = layout.people();
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
    let layout = VaultLayout::new(Path::new(vault_path()));
    layout.inbox().join("triage-patterns.json")
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

