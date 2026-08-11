use chrono::{Local, NaiveDate};
use std::fs;
use std::path::Path;

use pasta_common::vault::{vault_path, VaultLayout};

pub fn run() {
    archive_done_tasks();
    archive_stale_tasks();
    deduplicate_tasks();
    backfill_issue_tags();
    close_merged_mr_tasks();
    flag_stale_tasks();
    check_project_integrity();
    cleanup_old_logs();
    update_triage_patterns();
    route_new_tasks_sync();
}

/// Give Linear-sourced tasks an `issue:` frontmatter field (e.g. `AD-359`),
/// derived from `source_url`, so the issue key is queryable without parsing the URL.
fn backfill_issue_tags() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let mut tagged = 0;

    for path in walk_md_files(&tasks_dir) {
        let Ok(content) = fs::read_to_string(&path) else { continue; };
        if !has_frontmatter_value(&content, "source", "linear") { continue; }
        if frontmatter_value(&content, "issue").is_some_and(|v| !v.is_empty()) { continue; }
        let Some(url) = frontmatter_value(&content, "source_url") else { continue; };
        let Some(issue_id) = extract_work_item_key(&url).and_then(|k| k.strip_prefix("lin:").map(str::to_string)) else { continue; };

        let updated = pasta_common::vault::set_frontmatter(&content, "issue", &issue_id);
        fs::write(&path, updated).ok();
        tagged += 1;
    }

    if tagged > 0 {
        crate::util::log("vault-manager", &format!("tagged {} linear tasks with issue id", tagged));
    }
}

fn archive_done_tasks() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let archive_dir = layout.archive();
    fs::create_dir_all(&archive_dir).ok();

    for path in walk_md_files(&tasks_dir) {
        if let Ok(content) = fs::read_to_string(&path) {
            if has_frontmatter_value(&content, "status", "done") {
                let dest = archive_dir.join(path.file_name().unwrap_or_default());
                fs::rename(&path, dest).ok();
            }
        }
    }
}

fn archive_stale_tasks() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let archive_dir = layout.stale_archive();
    fs::create_dir_all(&archive_dir).ok();

    let mut archived = 0;
    for path in walk_md_files(&tasks_dir) {
        if let Ok(content) = fs::read_to_string(&path) {
            if content.contains("\nstale: true") {
                let dest = archive_dir.join(path.file_name().unwrap_or_default());
                if fs::rename(&path, &dest).is_ok() {
                    archived += 1;
                }
            }
        }
    }
    if archived > 0 {
        crate::util::log("vault-manager", &format!("archived {} stale tasks", archived));
    }
}

/// Find tasks that reference the same work item (MR or Linear issue) and merge them.
/// Keeps the task with more content; archives the duplicate.
fn deduplicate_tasks() {
    use std::collections::HashMap;

    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let archive_dir = layout.stale_archive();
    fs::create_dir_all(&archive_dir).ok();

    let files = walk_md_files(&tasks_dir);

    // Build a map of work-item key → list of (path, content-length)
    let mut work_items: HashMap<String, Vec<(std::path::PathBuf, usize)>> = HashMap::new();

    for path in &files {
        let Ok(content) = fs::read_to_string(path) else { continue; };

        // Extract work item keys from source_url and title
        let mut keys: Vec<String> = Vec::new();

        if let Some(url) = frontmatter_value(&content, "source_url") {
            if !url.is_empty() {
                // Normalize: extract MR or issue identifier from URL
                if let Some(key) = extract_work_item_key(&url) {
                    keys.push(key);
                }
            }
        }

        // Also extract MR/issue references from the filename
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
        if let Some(key) = extract_mr_from_name(&stem) {
            keys.push(key);
        }

        for key in keys {
            work_items.entry(key).or_default().push((path.clone(), content.len()));
        }
    }

    // For each work item with multiple files, keep the longest, archive the rest
    let mut deduped = 0;
    for (_key, mut entries) in work_items {
        if entries.len() <= 1 { continue; }

        // Sort by content length descending — keep the richest file
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        // Archive all but the first (richest)
        for (path, _) in &entries[1..] {
            let dest = archive_dir.join(path.file_name().unwrap_or_default());
            if !dest.exists()
                && fs::rename(path, &dest).is_ok() {
                    deduped += 1;
                }
        }
    }

    if deduped > 0 {
        crate::util::log("vault-manager", &format!("deduplicated {} task files", deduped));
    }
}

/// Extract a normalized work-item key from a URL.
/// e.g. "https://gitlab.com/readpeak/mononode/-/merge_requests/2955" → "gl:readpeak/mononode!2955"
/// e.g. "https://linear.app/readpeak/issue/AD-360/..." → "lin:AD-360"
fn extract_work_item_key(url: &str) -> Option<String> {
    let url = url.trim_matches('"');
    if url.contains("merge_requests/") {
        let after_host = url.strip_prefix("https://gitlab.com/")?;
        let (project, rest) = after_host.split_once("/-/merge_requests/")?;
        let iid = rest.split(&['/', '?', '#'][..]).next()?;
        Some(format!("gl:{}!{}", project, iid))
    } else if url.contains("linear.app") && url.contains("/issue/") {
        let after_issue = url.split("/issue/").nth(1)?;
        let identifier = after_issue.split('/').next()?;
        Some(format!("lin:{}", identifier.to_uppercase()))
    } else {
        None
    }
}

/// Extract MR reference from a task filename.
/// e.g. "resolve-discussions-mr-2955-vitest" → "mr:2955"
fn extract_mr_from_name(name: &str) -> Option<String> {
    // Look for pattern: mr-NNNN or mr-NN
    let parts: Vec<&str> = name.split('-').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "mr" || *part == "mrs" {
            if let Some(next) = parts.get(i + 1) {
                if next.chars().all(|c| c.is_ascii_digit()) && next.len() >= 2 {
                    return Some(format!("mr:{}", next));
                }
            }
        }
    }
    None
}

fn close_merged_mr_tasks() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();

    for path in walk_md_files(&tasks_dir) {
        let Ok(content) = fs::read_to_string(&path) else { continue; };

        // Only check open gitlab tasks with a source_url
        if !has_frontmatter_value(&content, "source", "gitlab") { continue; }
        if !has_frontmatter_value(&content, "status", "todo") { continue; }
        let Some(url) = frontmatter_value(&content, "source_url") else { continue; };
        if !url.contains("merge_requests") { continue; }

        // Extract project and MR IID from URL
        let Some((project, iid)) = parse_mr_url(&url) else { continue; };

        // Check MR state via glab
        let output = std::process::Command::new("glab")
            .args(["api", &format!("projects/{}/merge_requests/{}", project, iid)])
            .output()
            .ok();
        let Some(output) = output else { continue; };
        if !output.status.success() { continue; }

        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else { continue; };
        let state = json.get("state").and_then(|s| s.as_str()).unwrap_or("");

        if state == "merged" || state == "closed" {
            let today = Local::now().format("%Y-%m-%d").to_string();
            let updated = content.replacen("status: todo", "status: done", 1);
            let updated = if updated.contains("\ncompleted:") {
                updated
            } else {
                updated.replacen("status: done", &format!("status: done\ncompleted: {today}"), 1)
            };
            fs::write(&path, updated).ok();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            crate::util::log("vault-manager", &format!("closed merged MR task: {name}"));
        }
    }
}

/// Parse a GitLab MR URL into (url-encoded project path, iid)
fn parse_mr_url(url: &str) -> Option<(String, &str)> {
    // https://gitlab.com/readpeak/project/-/merge_requests/123
    let url = url.trim_matches('"');
    let after_host = url.strip_prefix("https://gitlab.com/")?;
    let (project_path, rest) = after_host.split_once("/-/merge_requests/")?;
    let iid = rest.split(&['/', '?', '#'][..]).next()?;
    let encoded = project_path.replace('/', "%2F");
    Some((encoded, iid))
}

fn flag_stale_tasks() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let cutoff = Local::now().date_naive() - chrono::Duration::days(14);

    for path in walk_md_files(&tasks_dir) {
        if let Ok(content) = fs::read_to_string(&path) {
            // Remove stale flag if task now has a due date or is no longer todo
            if content.contains("\nstale: true") {
                let has_due = frontmatter_value(&content, "due").is_some_and(|v| !v.is_empty());
                let not_todo = !has_frontmatter_value(&content, "status", "todo");
                if has_due || not_todo {
                    let updated = content.replace("\nstale: true", "");
                    fs::write(&path, updated).ok();
                    continue;
                }
            }

            if !has_frontmatter_value(&content, "status", "todo") { continue; }
            if frontmatter_value(&content, "due").is_some_and(|v| !v.is_empty()) { continue; }
            let is_stale = frontmatter_value(&content, "created")
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
                .is_some_and(|date| date < cutoff);
            if is_stale && !content.contains("\nstale: true") {
                let updated = content.replacen("status: todo", "status: todo\nstale: true", 1);
                fs::write(&path, updated).ok();
            }
        }
    }
}

fn check_project_integrity() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let projects_dir = layout.projects();
    let tasks_dir = layout.tasks();

    let Ok(project_entries) = fs::read_dir(&projects_dir) else { return };

    let mut task_projects: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(proj) = frontmatter_value(&content, "project") {
                        let proj = proj.replace("[[", "").replace("]]", "");
                        if !proj.is_empty() { task_projects.push(proj); }
                    }
                }
            }
        }
    }

    for entry in project_entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            let project_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if !task_projects.iter().any(|t| t == &project_name) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if !content.contains("⚠️ No active next action") {
                        let updated = format!("{}\n\n> ⚠️ No active next action\n", content.trim_end());
                        fs::write(&path, updated).ok();
                    }
                }
            } else if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("> ⚠️ No active next action") {
                    let updated = content.replace("\n\n> ⚠️ No active next action\n", "");
                    fs::write(&path, updated).ok();
                }
            }
        }
    }
}

fn has_frontmatter_value(content: &str, key: &str, value: &str) -> bool {
    pasta_common::vault::has_frontmatter_value(content, key, value)
}

/// Recursively walk a directory and return all `.md` file paths (skipping Kanban board files).
fn walk_md_files(dir: &Path) -> Vec<std::path::PathBuf> {
    pasta_common::vault::walk_md_files(dir)
}

fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    pasta_common::vault::frontmatter_value(content, key)
}

fn cleanup_old_logs() {
    let log_dir = crate::util::log_dir();
    let Ok(entries) = fs::read_dir(&log_dir) else { return };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 86400);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "log") {
            if let Ok(meta) = fs::metadata(&path) {
                if meta.modified().ok().is_some_and(|m| m < cutoff) {
                    fs::remove_file(&path).ok();
                }
            }
        }
    }
}

fn update_triage_patterns() {
    use pasta_common::vault::{load_triage_patterns, save_triage_patterns, TriagePattern};

    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let column_tags = pasta_common::vault::COLUMN_TAGS;
    let mut patterns = load_triage_patterns();
    let today = Local::now().format("%Y-%m-%d").to_string();

    // Decay: reduce hits for patterns not updated in 30+ days
    let cutoff = Local::now().date_naive() - chrono::Duration::days(30);
    patterns.retain_mut(|p| {
        if let Ok(d) = NaiveDate::parse_from_str(&p.last_updated, "%Y-%m-%d") {
            if d < cutoff {
                p.hits = p.hits.saturating_sub(1);
                p.last_updated = today.clone();
            }
        }
        p.hits > 0
    });

    // Scan tasks for source + column tag combinations
    let Ok(entries) = fs::read_dir(&tasks_dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md") { continue; }
        let Ok(content) = fs::read_to_string(&path) else { continue; };

        let source = frontmatter_value(&content, "source").unwrap_or_default();
        if source.is_empty() { continue; }

        // Find which column tag this task has
        let fm = content.strip_prefix("---").and_then(|r| r.split_once("---").map(|(f, _)| f)).unwrap_or("");
        let tags_line = fm.lines().find(|l| l.starts_with("tags:")).unwrap_or("");
        let column = column_tags.iter().find(|&&col| tags_line.contains(col));
        let Some(column) = column else { continue; };

        // Extract keywords from title
        let title = frontmatter_value(&content, "title").unwrap_or_default();
        let keywords: Vec<String> = title.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3)
            .take(3)
            .map(|s| s.to_string())
            .collect();
        if keywords.is_empty() { continue; }

        // Find matching pattern or update
        let matched = patterns.iter_mut().find(|p| {
            p.source == source && p.keywords.iter().any(|kw| keywords.contains(kw))
        });

        match matched {
            Some(p) => {
                if p.column == *column {
                    p.hits += 1;
                } else {
                    p.misses += 1;
                }
                p.last_updated = today.clone();
            }
            None => {
                // Don't create rule yet — track in a simple way
                // Check if we've seen this source+keyword combo enough times
                let existing_count = patterns.iter()
                    .filter(|p| p.source == source)
                    .count();
                if existing_count < 20 {
                    patterns.push(TriagePattern {
                        source: source.clone(),
                        keywords,
                        column: column.to_string(),
                        hits: 1,
                        misses: 0,
                        last_updated: today.clone(),
                    });
                }
            }
        }
    }

    save_triage_patterns(&patterns);
}

fn find_initiatives() -> Vec<(String, String)> {
    // Returns (name, description) for each initiative — identified by having a matching subfolder
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let mut initiatives = Vec::new();
    let Ok(entries) = fs::read_dir(&tasks_dir) else { return initiatives };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md") { continue; }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let subfolder = tasks_dir.join(&stem);
        if subfolder.is_dir() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            // Strip frontmatter for description
            let desc = content.strip_prefix("---")
                .and_then(|r| r.split_once("---").map(|(_, after)| after.trim()))
                .unwrap_or(&content);
            initiatives.push((stem, desc.to_string()));
        }
    }
    initiatives
}

fn route_new_tasks_sync() {
    // Synchronous wrapper — routes tasks using simple keyword matching against initiatives
    // (LanceDB search requires async; for vault-maintenance we use keyword fallback)
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let initiatives = find_initiatives();
    if initiatives.is_empty() { return; }

    let Ok(entries) = fs::read_dir(&tasks_dir) else { return };
    let mut moved = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md") { continue; }
        if path.is_dir() { continue; }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        // Skip initiative files themselves
        if initiatives.iter().any(|(name, _)| name == &stem) { continue; }

        let content = fs::read_to_string(&path).unwrap_or_default();
        // Pending tasks stay in Tasks/ until approved so they remain in the queue.
        if has_frontmatter_value(&content, "status", pasta_common::vault::STATUS_PENDING) { continue; }
        let title = frontmatter_value(&content, "title").unwrap_or_else(|| stem.clone());
        let project = frontmatter_value(&content, "project").unwrap_or_default()
            .replace("[[", "").replace("]]", "").replace("1. Projects/", "");

        // Match by project field first (deterministic), then by keyword similarity
        let matched_initiative = initiatives.iter().find(|(name, _)| {
            if !project.is_empty() {
                return match_project_to_initiative(&project, name);
            }
            false
        }).or_else(|| {
            // Keyword fallback: check if title words match initiative description
            let title_lower = title.to_lowercase();
            initiatives.iter().find(|(_, desc)| {
                let desc_lower = desc.to_lowercase();
                let title_words: Vec<&str> = title_lower.split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() > 3).collect();
                let matches = title_words.iter().filter(|w| desc_lower.contains(*w)).count();
                matches >= 2
            })
        });

        if let Some((initiative_name, _)) = matched_initiative {
            let dest = tasks_dir.join(initiative_name).join(path.file_name().unwrap());
            if !dest.exists() {
                fs::rename(&path, &dest).ok();
                moved += 1;
            }
        }
    }
    if moved > 0 {
        crate::util::log("route-tasks", &format!("moved {} tasks to initiative folders", moved));
    }
}

fn match_project_to_initiative(project: &str, initiative: &str) -> bool {
    let p = project.to_lowercase();
    let i = initiative.to_lowercase();
    match i.as_str() {
        "monitoring" => p.contains("monitoring") || p == "eks monitoring" || p == "sqs monitoring",
        "pipelines" => p.contains("pipeline") || p == "fix pipelines",
        "data platform" => p.contains("clickhouse") || p.contains("data"),
        "security & backup" => p.contains("backup") || p.contains("cost optimization"),
        "ad delivery" => p.contains("ad delivery") || p.contains("ad "),
        "ml platform" => p.contains("ml platform") || p.contains("sagemaker") || p.contains("profiling") || p.contains("semantic"),
        _ => p.contains(&i),
    }
}

/// Async version using LanceDB embeddings for semantic matching
pub async fn route_new_tasks_semantic() {
    let vault = Path::new(vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let initiatives = find_initiatives();
    if initiatives.is_empty() { return; }

    let Ok(entries) = fs::read_dir(&tasks_dir) else { return };
    let mut moved = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md") { continue; }
        if path.is_dir() { continue; }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        if initiatives.iter().any(|(name, _)| name == &stem) { continue; }

        let content = fs::read_to_string(&path).unwrap_or_default();
        if has_frontmatter_value(&content, "status", pasta_common::vault::STATUS_PENDING) { continue; }
        let title = frontmatter_value(&content, "title").unwrap_or_else(|| stem.clone());

        // Search kb-engine for closest initiative
        let results = match crate::kb_search::search(&title, Some("vault"), None, None, 3).await {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Find best matching initiative from results
        let matched = results.iter().find_map(|r| {
            initiatives.iter().find(|(name, _)| {
                r.path.to_lowercase().contains(&name.to_lowercase()) ||
                r.content.to_lowercase().contains(&name.to_lowercase())
            })
        });

        if let Some((initiative_name, _)) = matched {
            let dest = tasks_dir.join(initiative_name).join(path.file_name().unwrap());
            if !dest.exists() {
                fs::rename(&path, &dest).ok();
                moved += 1;
            }
        }
    }
    if moved > 0 {
        crate::util::log("route-tasks", &format!("semantically routed {} tasks", moved));
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    // Tests for extract_work_item_key parser
    #[test]
    fn test_extract_work_item_key_linear_issue_url() {
        // Example: "https://linear.app/readpeak/issue/AD-360/..." → "lin:AD-360"
        let url = "https://linear.app/readpeak/issue/AD-360/some-title-here";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("lin:AD-360".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_gitlab_mr_url() {
        // Example: "https://gitlab.com/readpeak/mononode/-/merge_requests/2955" → "gl:readpeak/mononode!2955"
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/2955";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("gl:readpeak/mononode!2955".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_with_query_params() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/2955?view=inline&pos=0";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("gl:readpeak/mononode!2955".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_with_fragment() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/2955#note_123456";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("gl:readpeak/mononode!2955".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_nested_project() {
        let url = "https://gitlab.com/readpeak/group/subgroup/project/-/merge_requests/42";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("gl:readpeak/group/subgroup/project!42".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_rejects_unsupported_urls() {
        let url = "https://github.com/user/repo/issues/123";
        let result = extract_work_item_key(url);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_work_item_key_with_trailing_slash() {
        let url = "https://linear.app/readpeak/issue/AD-360/";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("lin:AD-360".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_strips_quotes() {
        let url = "\"https://linear.app/readpeak/issue/AD-360/\"";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("lin:AD-360".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_linear_mixed_case() {
        let url = "https://linear.app/readpeak/issue/ABC-123/title";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("lin:ABC-123".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_linear_short() {
        let url = "https://linear.app/readpeak/issue/PROJ-1";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("lin:PROJ-1".to_string()));
    }

    #[test]
    fn test_extract_work_item_key_long_iid() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/1234567890";
        let result = extract_work_item_key(url);
        assert_eq!(result, Some("gl:readpeak/mononode!1234567890".to_string()));
    }

    // Tests for extract_mr_from_name parser
    #[test]
    fn test_extract_mr_from_name_simple_mr() {
        let name = "resolve-discussions-mr-2955-vitest";
        let result = extract_mr_from_name(name);
        assert_eq!(result, Some("mr:2955".to_string()));
    }

    #[test]
    fn test_extract_mr_from_name_mrs_plural() {
        let name = "fix-mrs-1234-bug";
        let result = extract_mr_from_name(name);
        assert_eq!(result, Some("mr:1234".to_string()));
    }

    #[test]
    fn test_extract_mr_from_name_single_digit() {
        let name = "mr-1-fix";
        let result = extract_mr_from_name(name);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_mr_from_name_two_digits() {
        let name = "mr-12-fix";
        let result = extract_mr_from_name(name);
        assert_eq!(result, Some("mr:12".to_string()));
    }

    #[test]
    fn test_extract_mr_from_name_three_digits() {
        let name = "fix-tasks-mr-345";
        let result = extract_mr_from_name(name);
        assert_eq!(result, Some("mr:345".to_string()));
    }

    #[test]
    fn test_extract_mr_from_name_at_start() {
        let name = "mr-999-early-bird";
        let result = extract_mr_from_name(name);
        assert_eq!(result, Some("mr:999".to_string()));
    }

    #[test]
    fn test_extract_mr_from_name_at_end() {
        let name = "some-task-mr-42";
        let result = extract_mr_from_name(name);
        assert_eq!(result, Some("mr:42".to_string()));
    }

    #[test]
    fn test_extract_mr_from_name_no_mr_pattern() {
        let name = "some-task-without-mr";
        let result = extract_mr_from_name(name);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_mr_from_name_mixed_case() {
        let name = "mr-Mr-123";
        let result = extract_mr_from_name(name);
        assert_eq!(result, None);
    }

    // Tests for parse_mr_url parser
    #[test]
    fn test_parse_mr_url_standard() {
        let url = "https://gitlab.com/readpeak/project/-/merge_requests/123";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fproject".to_string(), "123")));
    }

    #[test]
    fn test_parse_mr_url_nested_project() {
        let url = "https://gitlab.com/readpeak/group/subgroup/project/-/merge_requests/42";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fgroup%2Fsubgroup%2Fproject".to_string(), "42")));
    }

    #[test]
    fn test_parse_mr_url_with_query() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/2955?view=inline&pos=0";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fmononode".to_string(), "2955")));
    }

    #[test]
    fn test_parse_mr_url_with_fragment() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/2955#note_123456";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fmononode".to_string(), "2955")));
    }

    #[test]
    fn test_parse_mr_url_with_trailing_slash() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/2955/";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fmononode".to_string(), "2955")));
    }

    #[test]
    fn test_parse_mr_url_rejects_non_mr() {
        let url = "https://gitlab.com/readpeak/mononode/issues/123";
        let result = parse_mr_url(url);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_mr_url_rejects_github() {
        let url = "https://github.com/user/repo/pull/123";
        let result = parse_mr_url(url);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_mr_url_strips_quotes() {
        let url = "\"https://gitlab.com/readpeak/mononode/-/merge_requests/2955\"";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fmononode".to_string(), "2955")));
    }

    #[test]
    fn test_parse_mr_url_long_iid() {
        let url = "https://gitlab.com/readpeak/mononode/-/merge_requests/1234567890";
        let result = parse_mr_url(url);
        assert_eq!(result, Some(("readpeak%2Fmononode".to_string(), "1234567890")));
    }
}
